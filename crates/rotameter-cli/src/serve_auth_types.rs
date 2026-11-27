#![forbid(unsafe_code)]
//! Feature-independent UI and operator auth settings types for `rotameter serve`.

use std::path::PathBuf;

/// Feature-independent Web UI serving mode (`--ui=on|off`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UiModeSetting {
    /// Disabled by default.
    #[default]
    Off,
    /// Enabled (`--ui=on`).
    On,
}

/// Feature-independent static Web UI serving settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiSettings {
    /// Whether the static Web UI fallback is mounted.
    pub mode: UiModeSetting,
    /// Optional runtime directory overriding embedded `ui/dist/` assets.
    pub dir: Option<PathBuf>,
}

/// Feature-independent OIDC provider settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcSettings {
    /// OIDC issuer URL.
    pub issuer_url: String,
    /// OIDC client ID.
    pub client_id: String,
    /// OIDC client secret.
    pub client_secret: String,
    /// OIDC callback redirect URI.
    pub redirect_uri: String,
    /// Requested OIDC scopes.
    pub scopes: Vec<String>,
    /// Optional exact email allowlist.
    pub allowed_emails: Vec<String>,
    /// Optional domain allowlist.
    pub allowed_domains: Vec<String>,
}

/// Feature-independent operator auth settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorAuthSettings {
    /// Local admin username (`"admin"` default).
    pub admin_username: String,
    /// Optional plaintext local admin password.
    pub admin_password: Option<String>,
    /// Optional pre-hashed local admin password.
    pub admin_password_hash: Option<String>,
    /// Optional session cookie HMAC secret.
    pub session_secret: Option<String>,
    /// Session lifetime in seconds (`86_400` default).
    pub session_ttl_secs: u64,
    /// Whether Cloudflare Access headers are trusted.
    pub cf_access_trusted: bool,
    /// Optional Cloudflare Access team domain.
    pub cf_access_team_domain: Option<String>,
    /// Optional Cloudflare Access audience tag.
    pub cf_access_aud: Option<String>,
    /// Optional OIDC configuration.
    pub oidc: Option<OidcSettings>,
}

impl Default for OperatorAuthSettings {
    fn default() -> Self {
        Self {
            admin_username: "admin".to_string(),
            admin_password: None,
            admin_password_hash: None,
            session_secret: None,
            session_ttl_secs: 86_400,
            cf_access_trusted: false,
            cf_access_team_domain: None,
            cf_access_aud: None,
            oidc: None,
        }
    }
}

#[cfg(all(feature = "server", feature = "sqlite"))]
pub(crate) fn into_server_auth_and_ui(
    auth: OperatorAuthSettings,
    ui: UiSettings,
) -> (
    rotameter_server::OperatorAuthConfig,
    rotameter_server::UiConfig,
) {
    use rotameter_server::{OidcConfig, OperatorAuthConfig, UiConfig, UiMode};

    let op = OperatorAuthConfig {
        admin_username: auth.admin_username,
        admin_password: auth.admin_password,
        admin_password_hash: auth.admin_password_hash,
        session_secret: auth.session_secret,
        session_ttl_secs: auth.session_ttl_secs,
        cf_access_trusted: auth.cf_access_trusted,
        cf_access_team_domain: auth.cf_access_team_domain,
        cf_access_aud: auth.cf_access_aud,
        oidc: auth.oidc.map(|o| OidcConfig {
            issuer_url: o.issuer_url,
            client_id: o.client_id,
            client_secret: o.client_secret,
            redirect_uri: o.redirect_uri,
            scopes: o.scopes,
            allowed_emails: o.allowed_emails,
            allowed_domains: o.allowed_domains,
            authorization_endpoint: None,
            token_endpoint: None,
            userinfo_endpoint: None,
        }),
    };
    let ui_cfg = UiConfig {
        mode: match ui.mode {
            UiModeSetting::Off => UiMode::Off,
            UiModeSetting::On => UiMode::On,
        },
        dir: ui.dir,
    };
    (op, ui_cfg)
}
