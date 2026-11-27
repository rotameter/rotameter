#![forbid(unsafe_code)]
//! TOML config loading and primitive flag/env parsers for `rotameter serve`.

use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::exit::CliError;
use crate::serve_cmd::{SaltStoreSetting, ServeRoleSetting};

#[derive(Debug, Default, Deserialize)]
pub(crate) struct RootTomlConfig {
    #[serde(default)]
    pub(crate) server: Option<ServerTomlSection>,
    #[serde(default)]
    pub(crate) storage: Option<StorageTomlSection>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct ServerTomlSection {
    pub(crate) bind: Option<String>,
    pub(crate) db: Option<String>,
    pub(crate) bootstrap_admin_key: Option<String>,
    pub(crate) public_url: Option<String>,
    pub(crate) self_site: Option<bool>,
    pub(crate) self_site_id: Option<String>,
    pub(crate) geoip: Option<String>,
    pub(crate) mmdb_path: Option<PathBuf>,
    pub(crate) trusted_proxy: Option<bool>,
    pub(crate) role: Option<String>,
    pub(crate) salt_store: Option<String>,
    pub(crate) enable_mcp_http: Option<bool>,
    pub(crate) mcp_allow_admin: Option<bool>,
    pub(crate) ui: Option<String>,
    pub(crate) ui_dir: Option<PathBuf>,
    pub(crate) admin_username: Option<String>,
    pub(crate) admin_password: Option<String>,
    pub(crate) admin_password_hash: Option<String>,
    pub(crate) session_secret: Option<String>,
    pub(crate) session_ttl_secs: Option<u64>,
    pub(crate) cf_access_trusted: Option<bool>,
    pub(crate) cf_access_team_domain: Option<String>,
    pub(crate) cf_access_aud: Option<String>,
    pub(crate) oidc: Option<OidcTomlSection>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct OidcTomlSection {
    pub(crate) issuer_url: Option<String>,
    pub(crate) client_id: Option<String>,
    pub(crate) client_secret: Option<String>,
    pub(crate) redirect_uri: Option<String>,
    pub(crate) scopes: Option<Vec<String>>,
    pub(crate) allowed_emails: Option<Vec<String>>,
    pub(crate) allowed_domains: Option<Vec<String>>,
}

#[derive(Debug, Default, Deserialize)]
pub(crate) struct StorageTomlSection {
    pub(crate) db: Option<String>,
    pub(crate) control_db: Option<String>,
    pub(crate) analytics_db: Option<String>,
    pub(crate) duckdb_memory_limit: Option<String>,
    pub(crate) duckdb_threads: Option<u32>,
}

pub(crate) fn parse_role(raw: Option<&str>) -> Result<ServeRoleSetting, CliError> {
    match raw.map(str::to_ascii_lowercase).as_deref() {
        None | Some("all") => Ok(ServeRoleSetting::All),
        Some("ingest") => Ok(ServeRoleSetting::Ingest),
        Some("query") => Ok(ServeRoleSetting::Query),
        Some(other) => Err(CliError::usage(format!(
            "unsupported `--role` value `{other}` (expected all|ingest|query)"
        ))),
    }
}

pub(crate) fn parse_salt_store(raw: Option<&str>) -> Result<SaltStoreSetting, CliError> {
    match raw.map(str::to_ascii_lowercase).as_deref() {
        None | Some("db") => Ok(SaltStoreSetting::Db),
        Some("memory") => Ok(SaltStoreSetting::Memory),
        Some(other) => Err(CliError::usage(format!(
            "unsupported `--salt-store` value `{other}` (expected db|memory)"
        ))),
    }
}

pub(crate) fn read_toml_config(path: Option<&Path>) -> Result<RootTomlConfig, CliError> {
    let Some(p) = path else {
        return Ok(RootTomlConfig::default());
    };
    let raw = match std::fs::read_to_string(p) {
        Ok(s) => s,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(RootTomlConfig::default());
        }
        Err(err) => {
            return Err(CliError::runtime(format!(
                "failed to read config `{}`: {err}",
                p.display()
            )));
        }
    };
    toml::from_str(&raw)
        .map_err(|err| CliError::usage(format!("invalid TOML in `{}`: {err}", p.display())))
}

pub(crate) fn env_bool<F>(env_fn: &F, key: &str) -> Result<Option<bool>, CliError>
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

pub(crate) fn env_non_empty<F>(env_fn: &F, key: &str) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    env_fn(key).and_then(|v| clean_opt(Some(&v)))
}

pub(crate) fn env_u32<F>(env_fn: &F, key: &str) -> Result<Option<u32>, CliError>
where
    F: Fn(&str) -> Option<String>,
{
    let Some(raw) = env_non_empty(env_fn, key) else {
        return Ok(None);
    };
    let parsed: u32 = raw
        .parse()
        .map_err(|_| CliError::usage(format!("invalid integer value `{raw}` for {key}")))?;
    if parsed == 0 {
        return Err(CliError::usage(format!("{key} must be >= 1")));
    }
    Ok(Some(parsed))
}

pub(crate) fn clean_opt(raw: Option<&str>) -> Option<String> {
    raw.map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}
