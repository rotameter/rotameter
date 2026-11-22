#![forbid(unsafe_code)]
//! `rotameter serve` settings model and Tokio server runner.

use std::net::SocketAddr;
use std::path::PathBuf;

use crate::cli::{GlobalOpts, ServeArgs};
use crate::exit::CliError;

pub use crate::serve_resolve::{resolve_serve_settings, resolve_serve_settings_with};

/// Resolved server startup configuration independent of optional cargo features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeSettings {
    /// Socket address to bind (`127.0.0.1:8000` default).
    pub bind_addr: SocketAddr,
    /// `SQLite` database file path (`rotameter.db` default).
    pub db_path: String,
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
}

#[cfg(all(feature = "server", feature = "sqlite"))]
pub(crate) fn run_serve(global: &GlobalOpts, args: &ServeArgs) -> Result<(), CliError> {
    use rotameter_server::{GeoIpMode, ServerConfig};

    let settings = resolve_serve_settings(global, args)?;
    let config = ServerConfig {
        bind_addr: settings.bind_addr,
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
    let store = std::sync::Arc::new(
        crate::serve_store::SqliteServerStore::open(&settings.db_path, now_ms)
            .map_err(|err| CliError::runtime(format!("failed to open SQLite store: {err}")))?,
    );
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|err| CliError::runtime(format!("failed to build Tokio runtime: {err}")))?;
    rt.block_on(rotameter_server::serve(config, store))
        .map_err(|err| CliError::runtime(format!("server exited with error: {err}")))
}

#[cfg(not(all(feature = "server", feature = "sqlite")))]
pub(crate) fn run_serve(_global: &GlobalOpts, _args: &ServeArgs) -> Result<(), CliError> {
    Err(CliError::usage(
        "`rotameter serve` requires the `server` and `sqlite` cargo features",
    ))
}
