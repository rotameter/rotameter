#![forbid(unsafe_code)]
//! UI serving and operator auth (`session`, `oidc`, `cf_access`) settings resolution for `serve`.

use std::path::PathBuf;

use url::Url;

use crate::cli::ServeArgs;
use crate::exit::CliError;
use crate::serve_parse::{OidcTomlSection, ServerTomlSection, clean_opt, env_bool, env_non_empty};

#[cfg(all(feature = "server", feature = "sqlite"))]
pub(crate) use crate::serve_auth_types::into_server_auth_and_ui;
pub use crate::serve_auth_types::{OidcSettings, OperatorAuthSettings, UiModeSetting, UiSettings};

pub(crate) fn resolve_ui_settings<F>(
    args: &ServeArgs,
    env_fn: &F,
    srv: &ServerTomlSection,
) -> Result<UiSettings, CliError>
where
    F: Fn(&str) -> Option<String>,
{
    let raw_mode = clean_opt(args.ui.as_deref())
        .or_else(|| env_non_empty(env_fn, "ROTAMETER_UI"))
        .or_else(|| clean_opt(srv.ui.as_deref()));
    let mode = match raw_mode.map(|s| s.to_ascii_lowercase()).as_deref() {
        None | Some("off" | "false" | "0") => UiModeSetting::Off,
        Some("on" | "true" | "1") => UiModeSetting::On,
        Some(other) => {
            return Err(CliError::usage(format!(
                "unsupported `--ui` value `{other}` (expected on|off)"
            )));
        }
    };
    let dir = args
        .ui_dir
        .clone()
        .or_else(|| env_non_empty(env_fn, "ROTAMETER_UI_DIR").map(PathBuf::from))
        .or_else(|| srv.ui_dir.clone());
    Ok(UiSettings { mode, dir })
}

pub(crate) fn resolve_operator_auth<F>(
    args: &ServeArgs,
    env_fn: &F,
    srv: &ServerTomlSection,
) -> Result<OperatorAuthSettings, CliError>
where
    F: Fn(&str) -> Option<String>,
{
    let admin_username = clean_opt(args.admin_username.as_deref())
        .or_else(|| env_non_empty(env_fn, "ROTAMETER_ADMIN_USERNAME"))
        .or_else(|| clean_opt(srv.admin_username.as_deref()))
        .unwrap_or_else(|| "admin".to_string());
    let admin_password = clean_opt(args.admin_password.as_deref())
        .or_else(|| env_non_empty(env_fn, "ROTAMETER_ADMIN_PASSWORD"))
        .or_else(|| clean_opt(srv.admin_password.as_deref()));
    let admin_password_hash = clean_opt(args.admin_password_hash.as_deref())
        .or_else(|| env_non_empty(env_fn, "ROTAMETER_ADMIN_PASSWORD_HASH"))
        .or_else(|| clean_opt(srv.admin_password_hash.as_deref()));
    let session_secret = env_non_empty(env_fn, "ROTAMETER_SESSION_SECRET")
        .or_else(|| clean_opt(srv.session_secret.as_deref()));
    let session_ttl_secs = match env_non_empty(env_fn, "ROTAMETER_SESSION_TTL_SECS") {
        Some(raw) => raw.parse::<u64>().map_err(|_| {
            CliError::usage(format!(
                "invalid integer `{raw}` for ROTAMETER_SESSION_TTL_SECS"
            ))
        })?,
        None => srv.session_ttl_secs.unwrap_or(86_400),
    };
    let cf_access_trusted = env_bool(env_fn, "ROTAMETER_CF_ACCESS_TRUSTED")?
        .or(srv.cf_access_trusted)
        .unwrap_or(false);
    let cf_access_team_domain = env_non_empty(env_fn, "ROTAMETER_CF_ACCESS_TEAM_DOMAIN")
        .or_else(|| clean_opt(srv.cf_access_team_domain.as_deref()));
    let cf_access_aud = env_non_empty(env_fn, "ROTAMETER_CF_ACCESS_AUD")
        .or_else(|| clean_opt(srv.cf_access_aud.as_deref()));
    let oidc = resolve_oidc(env_fn, srv.oidc.as_ref())?;

    Ok(OperatorAuthSettings {
        admin_username,
        admin_password,
        admin_password_hash,
        session_secret,
        session_ttl_secs,
        cf_access_trusted,
        cf_access_team_domain,
        cf_access_aud,
        oidc,
    })
}

fn resolve_oidc<F>(
    env_fn: &F,
    cfg: Option<&OidcTomlSection>,
) -> Result<Option<OidcSettings>, CliError>
where
    F: Fn(&str) -> Option<String>,
{
    let issuer_url = env_non_empty(env_fn, "ROTAMETER_OIDC_ISSUER_URL")
        .or_else(|| cfg.and_then(|c| clean_opt(c.issuer_url.as_deref())));
    let Some(issuer_url) = issuer_url else {
        return Ok(None);
    };
    Url::parse(&issuer_url)
        .map_err(|e| CliError::usage(format!("invalid OIDC issuer URL `{issuer_url}`: {e}")))?;
    let client_id = env_non_empty(env_fn, "ROTAMETER_OIDC_CLIENT_ID")
        .or_else(|| cfg.and_then(|c| clean_opt(c.client_id.as_deref())))
        .ok_or_else(|| CliError::usage("OIDC requires client_id (`ROTAMETER_OIDC_CLIENT_ID`)"))?;
    let client_secret = env_non_empty(env_fn, "ROTAMETER_OIDC_CLIENT_SECRET")
        .or_else(|| cfg.and_then(|c| clean_opt(c.client_secret.as_deref())))
        .ok_or_else(|| {
            CliError::usage("OIDC requires client_secret (`ROTAMETER_OIDC_CLIENT_SECRET`)")
        })?;
    let redirect_uri = env_non_empty(env_fn, "ROTAMETER_OIDC_REDIRECT_URI")
        .or_else(|| cfg.and_then(|c| clean_opt(c.redirect_uri.as_deref())))
        .ok_or_else(|| {
            CliError::usage("OIDC requires redirect_uri (`ROTAMETER_OIDC_REDIRECT_URI`)")
        })?;
    Url::parse(&redirect_uri)
        .map_err(|e| CliError::usage(format!("invalid OIDC redirect_uri `{redirect_uri}`: {e}")))?;

    let scopes = env_non_empty(env_fn, "ROTAMETER_OIDC_SCOPES")
        .map(|s| split_csv(&s))
        .or_else(|| cfg.and_then(|c| c.scopes.clone()))
        .unwrap_or_else(|| vec!["openid".into(), "email".into(), "profile".into()]);
    let allowed_emails = env_non_empty(env_fn, "ROTAMETER_OIDC_ALLOWED_EMAILS")
        .map(|s| split_csv(&s))
        .or_else(|| cfg.and_then(|c| c.allowed_emails.clone()))
        .unwrap_or_default();
    let allowed_domains = env_non_empty(env_fn, "ROTAMETER_OIDC_ALLOWED_DOMAINS")
        .map(|s| split_csv(&s))
        .or_else(|| cfg.and_then(|c| c.allowed_domains.clone()))
        .unwrap_or_default();

    Ok(Some(OidcSettings {
        issuer_url,
        client_id,
        client_secret,
        redirect_uri,
        scopes,
        allowed_emails,
        allowed_domains,
    }))
}

fn split_csv(raw: &str) -> Vec<String> {
    raw.split([',', ' '])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
        .collect()
}
