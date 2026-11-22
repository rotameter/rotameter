#![forbid(unsafe_code)]
//! Profile configuration (`config.toml`), environment, and flag resolution.

mod types;

use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::time::Duration;

use url::Url;

use crate::error::ClientError;

use types::DEFAULT_TIMEOUT_SECS;
pub use types::{ClientConfig, ConfigFile, ConfigOverrides, ProfileConfig};

/// Returns the default per-user `config.toml` path (`$XDG_CONFIG_HOME` or `$HOME/.config`).
pub fn default_config_path() -> Option<PathBuf> {
    default_config_path_with(&|k| std::env::var(k).ok())
}

/// Resolves [`ClientConfig`] using `overrides`, process environment variables, and `config.toml`.
///
/// # Errors
///
/// Returns [`ClientError`] if reading/parsing `config.toml` fails, an explicit profile is missing,
/// no base URL is configured, or the resulting URL is invalid.
pub fn load(overrides: &ConfigOverrides) -> Result<ClientConfig, ClientError> {
    resolve_with(overrides, |k| std::env::var(k).ok())
}

/// Resolves [`ClientConfig`] with a caller-supplied environment lookup function for tests.
///
/// # Errors
///
/// Returns [`ClientError`] on unreadable/invalid `config.toml`, missing requested profile, missing
/// URL, or invalid URL syntax.
pub fn resolve_with<F>(overrides: &ConfigOverrides, env_fn: F) -> Result<ClientConfig, ClientError>
where
    F: Fn(&str) -> Option<String>,
{
    let config_path = overrides
        .config_path
        .clone()
        .or_else(|| env_non_empty(&env_fn, "ROTAMETER_CONFIG").map(PathBuf::from))
        .or_else(|| default_config_path_with(&env_fn));
    let config_file = match config_path.as_deref() {
        Some(path) => read_config_file(path)?,
        None => None,
    };

    let explicit_profile = clean_opt(overrides.profile.as_deref())
        .or_else(|| env_non_empty(&env_fn, "ROTAMETER_PROFILE"));
    let profile_name = explicit_profile.or_else(|| {
        config_file
            .as_ref()
            .and_then(|c| clean_opt(c.default_profile.as_deref()))
    });
    let profile = match profile_name.as_deref() {
        Some(name) => Some(
            config_file
                .as_ref()
                .and_then(|c| c.profiles.get(name))
                .ok_or_else(|| ClientError::ProfileNotFound(name.to_string()))?,
        ),
        None => None,
    };

    let raw_url = clean_opt(overrides.url.as_deref())
        .or_else(|| env_non_empty(&env_fn, "ROTAMETER_URL"))
        .or_else(|| profile.and_then(|p| clean_opt(p.url.as_deref())))
        .ok_or(ClientError::MissingUrl)?;

    Ok(ClientConfig {
        base_url: normalize_base_url(&raw_url)?,
        token: resolve_secret(
            overrides.token.as_deref(),
            "ROTAMETER_TOKEN",
            profile.and_then(|p| p.token_env.as_deref()),
            profile.and_then(|p| p.token.as_deref()),
            &env_fn,
        ),
        default_site: clean_opt(overrides.site.as_deref())
            .or_else(|| env_non_empty(&env_fn, "ROTAMETER_SITE"))
            .or_else(|| profile.and_then(|p| clean_opt(p.site.as_deref()))),
        cf_access_client_id: resolve_secret(
            overrides.cf_access_client_id.as_deref(),
            "CF_ACCESS_CLIENT_ID",
            profile.and_then(|p| p.cf_access_client_id_env.as_deref()),
            profile.and_then(|p| p.cf_access_client_id.as_deref()),
            &env_fn,
        ),
        cf_access_client_secret: resolve_secret(
            overrides.cf_access_client_secret.as_deref(),
            "CF_ACCESS_CLIENT_SECRET",
            profile.and_then(|p| p.cf_access_client_secret_env.as_deref()),
            profile.and_then(|p| p.cf_access_client_secret.as_deref()),
            &env_fn,
        ),
        timeout: overrides
            .timeout
            .unwrap_or_else(|| Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
    })
}

fn default_config_path_with<F>(env_fn: &F) -> Option<PathBuf>
where
    F: Fn(&str) -> Option<String>,
{
    if let Some(xdg) = env_non_empty(env_fn, "XDG_CONFIG_HOME") {
        return Some(PathBuf::from(xdg).join("rotameter").join("config.toml"));
    }
    env_non_empty(env_fn, "HOME").map(|h| {
        PathBuf::from(h)
            .join(".config")
            .join("rotameter")
            .join("config.toml")
    })
}

fn read_config_file(path: &Path) -> Result<Option<ConfigFile>, ClientError> {
    match std::fs::read_to_string(path) {
        Ok(raw) => toml::from_str(&raw)
            .map(Some)
            .map_err(|source| ClientError::ConfigParse {
                path: path.to_path_buf(),
                source,
            }),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ClientError::ConfigIo {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn resolve_secret<F>(
    override_val: Option<&str>,
    default_env: &str,
    profile_env: Option<&str>,
    profile_inline: Option<&str>,
    env_fn: &F,
) -> Option<String>
where
    F: Fn(&str) -> Option<String>,
{
    clean_opt(override_val)
        .or_else(|| env_non_empty(env_fn, default_env))
        .or_else(|| profile_env.and_then(|k| env_non_empty(env_fn, k.trim())))
        .or_else(|| clean_opt(profile_inline))
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

pub(crate) fn normalize_base_url(raw: &str) -> Result<Url, ClientError> {
    let trimmed = raw.trim();
    let mut parsed =
        Url::parse(trimmed).map_err(|err| ClientError::InvalidUrl(format!("{trimmed}: {err}")))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
        return Err(ClientError::InvalidUrl(format!(
            "expected http/https URL with host, got `{trimmed}`"
        )));
    }
    parsed.set_query(None);
    parsed.set_fragment(None);
    let stripped = parsed.path().trim_end_matches('/').to_string();
    parsed.set_path(&stripped);
    Ok(parsed)
}
