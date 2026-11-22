#![forbid(unsafe_code)]
//! Configuration file, override, and resolved client config types.

use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::normalize_base_url;
use crate::error::ClientError;

pub(super) const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Parsed `~/.config/rotameter/config.toml` document.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigFile {
    /// Optional default profile name used when `--profile` / `ROTAMETER_PROFILE` are unset.
    #[serde(default)]
    pub default_profile: Option<String>,
    /// Named connection profiles keyed by profile name.
    #[serde(default)]
    pub profiles: BTreeMap<String, ProfileConfig>,
}

/// Single profile entry under `[profiles.<name>]` in `config.toml`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileConfig {
    /// Base Rotameter server URL (e.g., `https://analytics.example.com`).
    pub url: Option<String>,
    /// Default site identifier (`site_...` or `slug`) associated with this profile.
    pub site: Option<String>,
    /// Inline API token (`rm_rd_...` or `rm_adm_...`); prefer `token_env` where possible.
    pub token: Option<String>,
    /// Environment variable name holding the Bearer token (defaults to `ROTAMETER_TOKEN`).
    pub token_env: Option<String>,
    /// Inline Cloudflare Access Service Token client ID.
    pub cf_access_client_id: Option<String>,
    /// Environment variable name holding the Cloudflare Access client ID.
    pub cf_access_client_id_env: Option<String>,
    /// Inline Cloudflare Access Service Token client secret.
    pub cf_access_client_secret: Option<String>,
    /// Environment variable name holding the Cloudflare Access client secret.
    pub cf_access_client_secret_env: Option<String>,
}

/// Explicit flag/caller overrides applied above environment variables and `config.toml`.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfigOverrides {
    /// Explicit base URL (`--url`).
    pub url: Option<String>,
    /// Explicit Bearer token (`--token`).
    pub token: Option<String>,
    /// Explicit profile name (`--profile`).
    pub profile: Option<String>,
    /// Explicit default site (`--site`).
    pub site: Option<String>,
    /// Explicit `CF-Access-Client-Id` (`--access-id`).
    pub cf_access_client_id: Option<String>,
    /// Explicit `CF-Access-Client-Secret` (`--access-secret`).
    pub cf_access_client_secret: Option<String>,
    /// Explicit `config.toml` path override.
    pub config_path: Option<PathBuf>,
    /// Explicit HTTP request timeout.
    pub timeout: Option<Duration>,
}

/// Resolved endpoint configuration consumed by [`crate::RotameterClient`].
#[derive(Clone, PartialEq, Eq)]
pub struct ClientConfig {
    /// Normalized base server URL (`http` or `https`, no trailing slash path).
    pub base_url: Url,
    /// Optional Bearer API token (`rm_ing_`, `rm_rd_`, or `rm_adm_`).
    pub token: Option<String>,
    /// Optional default site reference (`site_...` or `slug`).
    pub default_site: Option<String>,
    /// Optional Cloudflare Access Service Token client ID header value.
    pub cf_access_client_id: Option<String>,
    /// Optional Cloudflare Access Service Token client secret header value.
    pub cf_access_client_secret: Option<String>,
    /// Per-request HTTP timeout.
    pub timeout: Duration,
}

impl fmt::Debug for ClientConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientConfig")
            .field("base_url", &self.base_url.as_str())
            .field("token", &self.token.as_ref().map(|_| "[REDACTED]"))
            .field("default_site", &self.default_site)
            .field("cf_access_client_id", &self.cf_access_client_id)
            .field(
                "cf_access_client_secret",
                &self.cf_access_client_secret.as_ref().map(|_| "[REDACTED]"),
            )
            .field("timeout", &self.timeout)
            .finish()
    }
}

impl ClientConfig {
    /// Constructs a [`ClientConfig`] directly from a base server URL using default timeout.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::InvalidUrl`] if `base_url` is invalid or not `http`/`https`.
    pub fn new(base_url: &str) -> Result<Self, ClientError> {
        Ok(Self {
            base_url: normalize_base_url(base_url)?,
            token: None,
            default_site: None,
            cf_access_client_id: None,
            cf_access_client_secret: None,
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
        })
    }

    /// Attaches a Bearer token (`rm_ing_`, `rm_rd_`, or `rm_adm_`).
    #[must_use]
    pub fn with_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Attaches Cloudflare Access Service Token headers (`CF-Access-Client-Id` / `Secret`).
    #[must_use]
    pub fn with_cf_access(
        mut self,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        self.cf_access_client_id = Some(client_id.into());
        self.cf_access_client_secret = Some(client_secret.into());
        self
    }

    /// Sets the per-request HTTP timeout.
    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}
