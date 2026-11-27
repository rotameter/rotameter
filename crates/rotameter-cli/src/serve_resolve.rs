#![forbid(unsafe_code)]
//! Precedence resolution (`CLI > env > config [storage] / [server]`) for `rotameter serve`.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use url::Url;

use crate::cli::{GlobalOpts, ServeArgs};
use crate::context::{resolve_config_path, resolve_db_path};
use crate::exit::CliError;
use crate::serve_cmd::{McpHttpSettings, ServeSettings};
use crate::serve_parse::{
    clean_opt, env_bool, env_non_empty, env_u32, parse_role, parse_salt_store, read_toml_config,
};

/// Resolves `rotameter serve` settings with strict precedence (`CLI > env > config`).
///
/// # Errors
///
/// Returns [`CliError`] if config parsing, socket addresses, booleans, URLs, or `GeoIP` flags are invalid.
pub fn resolve_serve_settings(
    global: &GlobalOpts,
    args: &ServeArgs,
) -> Result<ServeSettings, CliError> {
    resolve_serve_settings_with(global, args, |k| std::env::var(k).ok())
}

/// Resolves `rotameter serve` settings using a caller-supplied env lookup for deterministic tests.
///
/// # Errors
///
/// Returns [`CliError`] on invalid TOML, invalid env/flag values, or unsupported `GeoIP` mode.
pub fn resolve_serve_settings_with<F>(
    global: &GlobalOpts,
    args: &ServeArgs,
    env_fn: F,
) -> Result<ServeSettings, CliError>
where
    F: Fn(&str) -> Option<String>,
{
    let root = read_toml_config(resolve_config_path(global, &env_fn).as_deref())?;
    let srv = root.server.unwrap_or_default();
    let sto = root.storage.unwrap_or_default();

    let bind_raw = clean_opt(args.bind.as_deref())
        .or_else(|| env_non_empty(&env_fn, "ROTAMETER_BIND"))
        .or_else(|| clean_opt(srv.bind.as_deref()));
    let bind_addr = match bind_raw {
        Some(raw) => raw
            .parse()
            .map_err(|err| CliError::usage(format!("invalid bind address `{raw}`: {err}")))?,
        None => SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000),
    };

    let cfg_control_db = sto
        .control_db
        .as_deref()
        .or(sto.db.as_deref())
        .or(srv.db.as_deref());
    let db_path = resolve_control_db_path(global, &env_fn, cfg_control_db);
    let analytics_db = clean_opt(global.analytics_db.as_deref())
        .or_else(|| env_non_empty(&env_fn, "ROTAMETER_ANALYTICS_DB"))
        .or_else(|| clean_opt(sto.analytics_db.as_deref()));

    let bootstrap_admin_key = clean_opt(args.bootstrap_admin_key.as_deref())
        .or_else(|| env_non_empty(&env_fn, "ROTAMETER_BOOTSTRAP_ADMIN_KEY"))
        .or_else(|| clean_opt(srv.bootstrap_admin_key.as_deref()));
    let public_url = clean_opt(args.public_url.as_deref())
        .or_else(|| env_non_empty(&env_fn, "ROTAMETER_PUBLIC_URL"))
        .or_else(|| clean_opt(srv.public_url.as_deref()));
    if let Some(raw_url) = public_url.as_deref() {
        Url::parse(raw_url)
            .map_err(|err| CliError::usage(format!("invalid `--public-url` `{raw_url}`: {err}")))?;
    }

    let cli_self_site = if args.no_self_site {
        Some(false)
    } else {
        args.self_site
    };
    let self_site_enabled = cli_self_site
        .or(env_bool(&env_fn, "ROTAMETER_SELF_SITE")?)
        .or(srv.self_site)
        .unwrap_or(true);
    let self_site_id = clean_opt(args.self_site_id.as_deref())
        .or_else(|| env_non_empty(&env_fn, "ROTAMETER_SELF_SITE_ID"))
        .or_else(|| clean_opt(srv.self_site_id.as_deref()));

    let geoip_raw = clean_opt(args.geoip.as_deref())
        .or_else(|| env_non_empty(&env_fn, "ROTAMETER_GEOIP"))
        .or_else(|| clean_opt(srv.geoip.as_deref()))
        .unwrap_or_else(|| "header".to_string());
    let geoip_mmdb = match geoip_raw.to_ascii_lowercase().as_str() {
        "header" => false,
        "mmdb" => true,
        other => {
            return Err(CliError::usage(format!(
                "unsupported `--geoip` mode `{other}`"
            )));
        }
    };
    let mmdb_path = args
        .mmdb_path
        .clone()
        .or_else(|| env_non_empty(&env_fn, "ROTAMETER_MMDB_PATH").map(PathBuf::from))
        .or(srv.mmdb_path);
    let trusted_proxy_headers = args
        .trusted_proxy
        .or(env_bool(&env_fn, "ROTAMETER_TRUSTED_PROXY")?)
        .or(srv.trusted_proxy)
        .unwrap_or(true);
    let role = parse_role(
        clean_opt(args.role.as_deref())
            .or_else(|| env_non_empty(&env_fn, "ROTAMETER_ROLE"))
            .or_else(|| clean_opt(srv.role.as_deref()))
            .as_deref(),
    )?;
    let salt_store = parse_salt_store(
        clean_opt(args.salt_store.as_deref())
            .or_else(|| env_non_empty(&env_fn, "ROTAMETER_SALT_STORE"))
            .or_else(|| clean_opt(srv.salt_store.as_deref()))
            .as_deref(),
    )?;
    let (duckdb_memory_limit, duckdb_threads) = resolve_duckdb_limits(args, &env_fn, &sto)?;
    let mcp_http = resolve_mcp_http(args, &env_fn, srv.enable_mcp_http, srv.mcp_allow_admin)?;

    Ok(ServeSettings {
        bind_addr,
        role,
        salt_store,
        db_path,
        analytics_db,
        bootstrap_admin_key,
        public_url,
        self_site_enabled,
        self_site_id,
        geoip_mmdb,
        mmdb_path,
        trusted_proxy_headers,
        duckdb_memory_limit,
        duckdb_threads,
        mcp_http,
    })
}

fn resolve_mcp_http<F>(
    args: &ServeArgs,
    env_fn: &F,
    cfg_enabled: Option<bool>,
    cfg_allow_admin: Option<bool>,
) -> Result<McpHttpSettings, CliError>
where
    F: Fn(&str) -> Option<String>,
{
    Ok(McpHttpSettings {
        enabled: args
            .enable_mcp_http
            .or(env_bool(env_fn, "ROTAMETER_ENABLE_MCP_HTTP")?)
            .or(cfg_enabled)
            .unwrap_or(false),
        allow_admin: args
            .mcp_allow_admin
            .or(env_bool(env_fn, "ROTAMETER_MCP_ALLOW_ADMIN")?)
            .or(cfg_allow_admin)
            .unwrap_or(false),
    })
}

fn resolve_duckdb_limits<F>(
    args: &ServeArgs,
    env_fn: &F,
    sto: &crate::serve_parse::StorageTomlSection,
) -> Result<(String, u32), CliError>
where
    F: Fn(&str) -> Option<String>,
{
    let memory_limit = clean_opt(args.duckdb_memory_limit.as_deref())
        .or_else(|| env_non_empty(env_fn, "ROTAMETER_DUCKDB_MEMORY_LIMIT"))
        .or_else(|| clean_opt(sto.duckdb_memory_limit.as_deref()))
        .unwrap_or_else(|| "256MB".to_string());
    let threads = args
        .duckdb_threads
        .or(env_u32(env_fn, "ROTAMETER_DUCKDB_THREADS")?)
        .or(sto.duckdb_threads)
        .unwrap_or(1);
    if threads == 0 {
        return Err(CliError::usage(
            "`--duckdb-threads` / `ROTAMETER_DUCKDB_THREADS` must be >= 1",
        ));
    }
    Ok((memory_limit, threads))
}

fn resolve_control_db_path<F>(global: &GlobalOpts, env_fn: &F, config_db: Option<&str>) -> String
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(cli_db) = clean_opt(global.db.as_deref()) {
        return cli_db;
    }
    if let Some(env_ctrl) = env_non_empty(env_fn, "ROTAMETER_CONTROL_DB") {
        return env_ctrl;
    }
    resolve_db_path(global, env_fn, config_db)
}
