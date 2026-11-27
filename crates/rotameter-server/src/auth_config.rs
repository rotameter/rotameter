#![forbid(unsafe_code)]
//! Configuration types for operator authentication (`session`, `oidc`, `cf_access`) and static UI serving.

use std::path::PathBuf;

/// Static Web UI serving mode (`--ui=on|off`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UiMode {
    /// Web UI disabled (`default`).
    #[default]
    Off,
    /// Web UI enabled (served from `--ui-dir` or embedded `feature = "ui"` assets).
    On,
}

/// Static Web UI serving configuration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UiConfig {
    /// Whether the static Web UI fallback is mounted (`off` by default).
    pub mode: UiMode,
    /// Optional runtime directory overriding embedded `ui/dist/` assets (`--ui-dir`).
    pub dir: Option<PathBuf>,
}

/// `OpenID` Connect (Authorization Code + PKCE) configuration for operator login.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OidcConfig {
    /// OIDC issuer URL (used for `.well-known/openid-configuration` discovery).
    pub issuer_url: String,
    /// Registered OIDC client ID.
    pub client_id: String,
    /// Registered OIDC client secret.
    pub client_secret: String,
    /// Callback redirect URI pointing to `/v1/auth/oidc/callback`.
    pub redirect_uri: String,
    /// Requested scopes (defaults to `["openid", "email", "profile"]`).
    pub scopes: Vec<String>,
    /// Optional allowlist of exact operator emails.
    pub allowed_emails: Vec<String>,
    /// Optional allowlist of email domains (e.g., `"example.com"`).
    pub allowed_domains: Vec<String>,
    /// Optional explicit authorization endpoint override (skips discovery when all 3 are set).
    pub authorization_endpoint: Option<String>,
    /// Optional explicit token endpoint override.
    pub token_endpoint: Option<String>,
    /// Optional explicit userinfo endpoint override.
    pub userinfo_endpoint: Option<String>,
}

/// Operator authentication configuration for `/v1/auth/*` and Web UI sessions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorAuthConfig {
    /// Local admin username (`"admin"` by default).
    pub admin_username: String,
    /// Optional plaintext local admin password (salted & hashed in memory on boot).
    pub admin_password: Option<String>,
    /// Optional pre-hashed local admin password (`sha256$<salt_hex>$<digest_hex>`).
    pub admin_password_hash: Option<String>,
    /// Optional HMAC secret for signing `rotameter_session` cookies (CSPRNG generated if unset).
    pub session_secret: Option<String>,
    /// Session cookie lifetime in seconds (`86_400` by default).
    pub session_ttl_secs: u64,
    /// Whether Cloudflare Access headers (`CF-Access-Authenticated-User-Email` / JWT) are trusted.
    pub cf_access_trusted: bool,
    /// Optional Cloudflare Access team domain (`https://<team>.cloudflareaccess.com`) for JWT checks.
    pub cf_access_team_domain: Option<String>,
    /// Optional Cloudflare Access Application Audience (`aud`) tag.
    pub cf_access_aud: Option<String>,
    /// Optional `OpenID` Connect provider configuration.
    pub oidc: Option<OidcConfig>,
}

impl Default for OperatorAuthConfig {
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
