#![forbid(unsafe_code)]
//! `rotameter serve` settings model and Tokio server runner.

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::cli::{GlobalOpts, ServeArgs};
use crate::exit::CliError;

pub use crate::serve_resolve::{resolve_serve_settings, resolve_serve_settings_with};

/// Feature-independent `rotameter serve` endpoint role.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ServeRoleSetting {
    /// Serve both ingest and query/admin endpoints.
    #[default]
    All,
    /// Serve ingest + collector + health endpoints only.
    Ingest,
    /// Serve query + realtime + admin + health endpoints only.
    Query,
}

/// Feature-independent rotating salt store strategy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SaltStoreSetting {
    /// Persist active salts in `ControlPlaneStore` with in-memory caching & pruning.
    #[default]
    Db,
    /// Ephemeral process-local in-memory rotating salts only.
    Memory,
}

/// Resolved server startup configuration independent of optional cargo features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeSettings {
    /// Socket address to bind (`127.0.0.1:8000` default).
    pub bind_addr: SocketAddr,
    /// Server endpoint role (`all`, `ingest`, or `query`).
    pub role: ServeRoleSetting,
    /// Rotating salt store mode (`db` or `memory`).
    pub salt_store: SaltStoreSetting,
    /// Control-plane `SQLite` database path or DSN (`rotameter.db` default).
    pub db_path: String,
    /// Optional analytics data-plane DSN overriding `db_path` for OLAP storage.
    pub analytics_db: Option<String>,
    /// Optional bootstrap admin token (`rm_adm_...`).
    pub bootstrap_admin_key: Option<String>,
    /// Optional public URL whose host seeds `self_site` slug.
    pub public_url: Option<String>,
    /// Whether local `self_site` first-boot initialization is enabled (`true` default).
    pub self_site_enabled: bool,
    /// Optional explicit `self_site` UID or slug override.
    pub self_site_id: Option<String>,
    /// Whether `MaxMind` `.mmdb` `GeoIP` lookup is active (`false` = header mode).
    pub geoip_mmdb: bool,
    /// Path to `.mmdb` database when `geoip_mmdb` is `true`.
    pub mmdb_path: Option<PathBuf>,
    /// Whether `CF-*` proxy headers are trusted on unauthenticated sockets.
    pub trusted_proxy_headers: bool,
    /// `DuckDB` `memory_limit` pragma value when analytics uses `duckdb://`.
    pub duckdb_memory_limit: String,
    /// `DuckDB` worker thread cap when analytics uses `duckdb://`.
    pub duckdb_threads: u32,
}

#[cfg(all(feature = "server", feature = "sqlite"))]
pub(crate) fn run_serve(global: &GlobalOpts, args: &ServeArgs) -> Result<(), CliError> {
    use std::sync::Arc;

    use rotameter_server::{GeoIpMode, SaltStoreMode, ServerConfig, ServerRole, ServerStores};

    use crate::serve_store::SqliteServerStore;
    use crate::storage_dsn::StorageDsn;

    let settings = resolve_serve_settings(global, args)?;
    let control_path = match StorageDsn::parse(&settings.db_path)? {
        StorageDsn::Sqlite(path) => path,
        StorageDsn::DuckDb(_) => {
            return Err(CliError::usage(
                "DuckDB cannot host the transactional control plane; use SQLite for `--db` / `--control-db`",
            ));
        }
    };
    let analytics_dsn = settings
        .analytics_db
        .as_deref()
        .map(StorageDsn::parse)
        .transpose()?;

    let config = ServerConfig {
        bind_addr: settings.bind_addr,
        role: match settings.role {
            ServeRoleSetting::All => ServerRole::All,
            ServeRoleSetting::Ingest => ServerRole::Ingest,
            ServeRoleSetting::Query => ServerRole::Query,
        },
        salt_store: match settings.salt_store {
            SaltStoreSetting::Db => SaltStoreMode::Db,
            SaltStoreSetting::Memory => SaltStoreMode::Memory,
        },
        bootstrap_admin_key: settings.bootstrap_admin_key,
        public_url: settings.public_url,
        self_site_enabled: settings.self_site_enabled,
        self_site_id: settings.self_site_id,
        trusted_proxy_headers: settings.trusted_proxy_headers,
        geoip_mode: if settings.geoip_mmdb {
            GeoIpMode::Mmdb
        } else {
            GeoIpMode::Header
        },
        mmdb_path: settings.mmdb_path,
        ..ServerConfig::default()
    };

    rotameter_server::init_tracing();
    let now_ms = crate::format::now_ms();
    let control = Arc::new(
        SqliteServerStore::open(&control_path, now_ms)
            .map_err(|err| CliError::runtime(format!("failed to open SQLite store: {err}")))?,
    );
    let stores = match analytics_dsn {
        None => ServerStores::unified(control),
        Some(StorageDsn::Sqlite(analytics_path)) => {
            let analytics = Arc::new(SqliteServerStore::open(&analytics_path, now_ms).map_err(
                |err| CliError::runtime(format!("failed to open analytics SQLite store: {err}")),
            )?);
            ServerStores::split(control, analytics)
        }
        Some(StorageDsn::DuckDb(analytics_path)) => {
            #[cfg(feature = "duckdb")]
            {
                let duck_cfg = rotameter_store_duckdb::DuckDbConfig {
                    memory_limit: Some(settings.duckdb_memory_limit),
                    threads: Some(settings.duckdb_threads),
                };
                let analytics = Arc::new(
                    crate::serve_duckdb::DuckDbServerStore::open(&analytics_path, &duck_cfg)
                        .map_err(|err| {
                            CliError::runtime(format!(
                                "failed to open analytics DuckDB store: {err}"
                            ))
                        })?,
                );
                ServerStores::split(control, analytics)
            }
            #[cfg(not(feature = "duckdb"))]
            {
                let _ = analytics_path;
                return Err(CliError::usage(
                    "DuckDB analytics backend requires `--features duckdb`",
                ));
            }
        }
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| CliError::runtime(format!("failed to build Tokio runtime: {err}")))?;
    rt.block_on(rotameter_server::serve(config, stores))
        .map_err(|err| CliError::runtime(format!("server exited with error: {err}")))
}

#[cfg(not(all(feature = "server", feature = "sqlite")))]
pub(crate) fn run_serve(_global: &GlobalOpts, _args: &ServeArgs) -> Result<(), CliError> {
    Err(CliError::usage(
        "`rotameter serve` requires the `server` and `sqlite` cargo features",
    ))
}
