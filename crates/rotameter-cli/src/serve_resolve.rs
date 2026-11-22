#![forbid(unsafe_code)]
//! Precedence resolution (`CLI > env > config [server]`) for `rotameter serve`.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use url::Url;

use crate::cli::{GlobalOpts, ServeArgs};
use crate::context::{resolve_config_path, resolve_db_path};
use crate::exit::CliError;
use crate::serve_cmd::ServeSettings;

#[derive(Debug, Default, Deserialize)]
struct RootTomlConfig {
    #[serde(default)]
    server: Option<ServerTomlSection>,
}

#[derive(Debug, Default, Deserialize)]
struct ServerTomlSection {
    bind: Option<String>,
    db: Option<String>,
    bootstrap_admin_key: Option<String>,
    public_url: Option<String>,
    self_site: Option<bool>,
    self_site_id: Option<String>,
    geoip: Option<String>,
    mmdb_path: Option<PathBuf>,
    trusted_proxy: Option<bool>,
}

/// Resolves `rotameter serve` settings with strict precedence (`CLI > env > config [server]`).
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
    let srv = read_server_section(resolve_config_path(global, &env_fn).as_deref())?;
    let bind_raw = clean_opt(args.bind.as_deref())
        .or_else(|| env_non_empty(&env_fn, "ROTAMETER_BIND"))
        .or_else(|| clean_opt(srv.bind.as_deref()));
    let bind_addr = match bind_raw {
        Some(raw) => raw
            .parse()
            .map_err(|err| CliError::usage(format!("invalid bind address `{raw}`: {err}")))?,
        None => SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000),
    };

    let db_path = resolve_db_path(global, &env_fn, srv.db.as_deref());
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

    Ok(ServeSettings {
        bind_addr,
        db_path,
        bootstrap_admin_key,
        public_url,
        self_site_enabled,
        self_site_id,
        geoip_mmdb,
        mmdb_path,
        trusted_proxy_headers,
    })
}

fn read_server_section(path: Option<&Path>) -> Result<ServerTomlSection, CliError> {
    let Some(p) = path else {
        return Ok(ServerTomlSection::default());
    };
    let raw = match std::fs::read_to_string(p) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ServerTomlSection::default());
        }
        Err(err) => {
            return Err(CliError::runtime(format!(
                "failed to read config `{}`: {err}",
                p.display()
            )));
        }
    };
    let parsed: RootTomlConfig = toml::from_str(&raw)
        .map_err(|err| CliError::usage(format!("invalid TOML in `{}`: {err}", p.display())))?;
    Ok(parsed.server.unwrap_or_default())
}

fn env_bool<F>(env_fn: &F, key: &str) -> Result<Option<bool>, CliError>
where
    F: Fn(&str) -> Option<String>,
{
    let Some(raw) = env_non_empty(env_fn, key) else {
        return Ok(None);
    };
    match raw.to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" => Ok(Some(true)),
        "false" | "0" | "no" => Ok(Some(false)),
        _ => Err(CliError::usage(format!(
            "invalid boolean value `{raw}` for {key}"
        ))),
    }
}

fn env_non_empty<F>(env_fn: &F, key: &str) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    env_fn(key).and_then(|v| clean_opt(Some(&v)))
}

fn clean_opt(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}
