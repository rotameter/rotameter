#![forbid(unsafe_code)]
//! Shared CLI client/store resolution and formatted output helpers.

use std::path::PathBuf;

use rotameter_client::BlockingRotameterClient;
use rotameter_client::config::{self, ConfigOverrides, default_config_path};
#[cfg(feature = "sqlite")]
use rotameter_core::id::OrgId;
use serde::Serialize;

use crate::cli::GlobalOpts;
use crate::exit::CliError;
#[cfg(feature = "sqlite")]
use crate::format::now_ms;
use crate::format::{OutputFormat, render_csv, render_json, render_table};

pub(crate) fn overrides_from_global(global: &GlobalOpts) -> ConfigOverrides {
    ConfigOverrides {
        url: global.url.clone(),
        token: global.token.clone(),
        profile: global.profile.clone(),
        site: None,
        cf_access_client_id: global.access_id.clone(),
        cf_access_client_secret: global.access_secret.clone(),
        config_path: global.config.clone(),
        timeout: None,
    }
}

pub(crate) fn remote_client(global: &GlobalOpts) -> Result<BlockingRotameterClient, CliError> {
    let cfg = config::load(&overrides_from_global(global))?;
    Ok(BlockingRotameterClient::new(cfg)?)
}

pub(crate) fn resolve_config_path(
    global: &GlobalOpts,
    env_fn: &impl Fn(&str) -> Option<String>,
) -> Option<PathBuf> {
    global
        .config
        .clone()
        .or_else(|| {
            env_fn("ROTAMETER_CONFIG")
                .filter(|s| !s.trim().is_empty())
                .map(PathBuf::from)
        })
        .or_else(default_config_path)
}

pub(crate) fn resolve_db_path(
    global: &GlobalOpts,
    env_fn: &impl Fn(&str) -> Option<String>,
    config_db: Option<&str>,
) -> String {
    global
        .db
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .or_else(|| env_fn("ROTAMETER_DB").filter(|s| !s.trim().is_empty()))
        .or_else(|| {
            config_db
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "rotameter.db".to_string())
}

pub(crate) fn prefer_local_db(global: &GlobalOpts) -> bool {
    let has_remote_override = global.url.is_some()
        || global.profile.is_some()
        || std::env::var("ROTAMETER_URL").is_ok_and(|v| !v.trim().is_empty())
        || std::env::var("ROTAMETER_PROFILE").is_ok_and(|v| !v.trim().is_empty());
    if has_remote_override {
        return false;
    }
    global.db.is_some() || std::env::var("ROTAMETER_DB").is_ok_and(|v| !v.trim().is_empty())
}

#[cfg(feature = "sqlite")]
pub(crate) fn open_local_store(
    global: &GlobalOpts,
) -> Result<(rotameter_store_sqlite::SqliteStore, i64, OrgId), CliError> {
    use rotameter_core::storage::ControlPlaneStore;
    use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

    let db_path = resolve_db_path(global, &|k| std::env::var(k).ok(), None);
    let now = now_ms();
    let store = SqliteStore::open(&db_path, SqliteConfig::default(), now)?;
    let org = store.ensure_default_org(now)?;
    Ok((store, now, org.org_id))
}

pub(crate) fn print_formatted<T: Serialize>(
    format: OutputFormat,
    headers: &[&str],
    rows: &[Vec<String>],
    json_payload: &T,
    quiet: bool,
) -> Result<(), CliError> {
    let text = match format {
        OutputFormat::Table => {
            if quiet && rows.is_empty() {
                String::new()
            } else {
                render_table(headers, rows)
            }
        }
        OutputFormat::Json => render_json(json_payload)?,
        OutputFormat::Csv => render_csv(headers, rows),
    };
    print!("{text}");
    Ok(())
}
