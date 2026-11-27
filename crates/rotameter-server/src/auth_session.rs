#![forbid(unsafe_code)]
//! Signed `HttpOnly` operator session cookies, password hashing, CSRF protection, and revocation.

use std::collections::HashMap;
use std::sync::Mutex;

use axum::http::HeaderMap;
use axum::http::header::{HOST, ORIGIN};
use hmac_sha256::{HMAC, Hash};
use rotameter_core::id::{OrgId, SiteId};
use rotameter_core::types::KeyScope;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::auth_codec::{
    PasswordVerifier, b64url_decode, b64url_encode, build_verifier, ct_eq, hex_encode,
};
use crate::auth_oidc::OidcManager;
use crate::config::{ServerConfig, ServerError};
use crate::error::ApiError;
use crate::state::fill_random;

pub(crate) const SESSION_COOKIE_NAME: &str = "rotameter_session";
pub(crate) const CSRF_COOKIE_NAME: &str = "rotameter_csrf";
const MAX_REVOKED_SIDS: usize = 4_096;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SessionClaims {
    pub(crate) sid: String,
    pub(crate) sub: String,
    pub(crate) org_id: OrgId,
    pub(crate) scope: KeyScope,
    #[serde(default)]
    pub(crate) site_id: Option<SiteId>,
    pub(crate) method: String,
    pub(crate) exp_ms: i64,
    pub(crate) csrf: String,
}

pub(crate) struct OperatorAuthState {
    session_key: [u8; 32],
    session_ttl_secs: u64,
    cookie_secure: bool,
    public_host: Option<String>,
    admin_username: String,
    admin_verifier: Option<PasswordVerifier>,
    pub(crate) cf_access_trusted: bool,
    pub(crate) cf_access_team_domain: Option<String>,
    pub(crate) cf_access_aud: Option<String>,
    revoked_sids: Mutex<HashMap<String, i64>>,
    pub(crate) oidc: Option<OidcManager>,
}

impl OperatorAuthState {
    pub(crate) fn from_config(config: &ServerConfig) -> Result<Self, ServerError> {
        let auth = &config.operator_auth;
        let session_key = match auth
            .session_secret
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            Some(secret) => Hash::hash(secret.as_bytes()),
            None => fill_random::<32>()
                .map_err(|e| ServerError::new(format!("cannot generate session key: {e}")))?,
        };
        let admin_verifier = build_verifier(auth)?;
        let cookie_secure = config
            .public_url
            .as_deref()
            .is_some_and(|u| u.trim().to_ascii_lowercase().starts_with("https://"));
        let public_host = config
            .public_url
            .as_deref()
            .and_then(|u| Url::parse(u).ok())
            .and_then(|u| u.host_str().map(str::to_ascii_lowercase));
        let oidc = auth.oidc.clone().map(OidcManager::new);
        Ok(Self {
            session_key,
            session_ttl_secs: auth.session_ttl_secs.max(60),
            cookie_secure,
            public_host,
            admin_username: auth.admin_username.clone(),
            admin_verifier,
            cf_access_trusted: auth.cf_access_trusted,
            cf_access_team_domain: auth.cf_access_team_domain.clone(),
            cf_access_aud: auth.cf_access_aud.clone(),
            revoked_sids: Mutex::new(HashMap::new()),
            oidc,
        })
    }

    pub(crate) fn has_local_admin(&self) -> bool {
        self.admin_verifier.is_some()
    }

    pub(crate) fn verify_admin_credentials(&self, username: &str, password: &str) -> bool {
        let Some(ref verifier) = self.admin_verifier else {
            return false;
        };
        if !ct_eq(username.as_bytes(), self.admin_username.as_bytes()) {
            return false;
        }
        let actual = HMAC::mac(password.as_bytes(), &verifier.salt);
        ct_eq(&actual, &verifier.digest)
    }

    pub(crate) fn issue_session(
        &self,
        sub: String,
        org_id: OrgId,
        scope: KeyScope,
        site_id: Option<SiteId>,
        method: &str,
        now_ms: i64,
    ) -> Result<(String, Vec<String>), ApiError> {
        let sid = hex_encode(&fill_random::<16>().map_err(ApiError::from)?);
        let csrf = hex_encode(&fill_random::<16>().map_err(ApiError::from)?);
        let ttl_ms = i64::try_from(self.session_ttl_secs)
            .unwrap_or(86_400)
            .saturating_mul(1_000);
        let claims = SessionClaims {
            sid,
            sub,
            org_id,
            scope,
            site_id,
            method: method.to_string(),
            exp_ms: now_ms.saturating_add(ttl_ms),
            csrf: csrf.clone(),
        };
        let payload = serde_json::to_vec(&claims).map_err(|e| ApiError::internal(e.to_string()))?;
        let encoded = b64url_encode(&payload);
        let sig = b64url_encode(&HMAC::mac(encoded.as_bytes(), self.session_key));
        let token = format!("{encoded}.{sig}");
        let sec = if self.cookie_secure { "; Secure" } else { "" };
        let max_age = self.session_ttl_secs;
        let cookies = vec![
            format!(
                "{SESSION_COOKIE_NAME}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{sec}"
            ),
            format!("{CSRF_COOKIE_NAME}={csrf}; Path=/; SameSite=Lax; Max-Age={max_age}{sec}"),
        ];
        Ok((csrf, cookies))
    }

    pub(crate) fn clear_cookie_headers(&self) -> Vec<String> {
        let sec = if self.cookie_secure { "; Secure" } else { "" };
        vec![
            format!("{SESSION_COOKIE_NAME}=; Path=/; HttpOnly; SameSite=Lax; Max-Age=0{sec}"),
            format!("{CSRF_COOKIE_NAME}=; Path=/; SameSite=Lax; Max-Age=0{sec}"),
        ]
    }

    pub(crate) fn verify_session_cookie(&self, raw: &str, now_ms: i64) -> Option<SessionClaims> {
        let (encoded, sig_b64) = raw.split_once('.')?;
        let sig = b64url_decode(sig_b64)?;
        let expected = HMAC::mac(encoded.as_bytes(), self.session_key);
        if !ct_eq(&sig, &expected) {
            return None;
        }
        let json_bytes = b64url_decode(encoded)?;
        let claims: SessionClaims = serde_json::from_slice(&json_bytes).ok()?;
        if claims.exp_ms <= now_ms {
            return None;
        }
        let guard = self.revoked_sids.lock().ok()?;
        if guard.get(&claims.sid).is_some_and(|&exp| exp > now_ms) {
            return None;
        }
        Some(claims)
    }

    pub(crate) fn revoke_session(&self, sid: &str, exp_ms: i64, now_ms: i64) {
        if let Ok(mut guard) = self.revoked_sids.lock() {
            guard.retain(|_, &mut exp| exp > now_ms);
            if guard.len() >= MAX_REVOKED_SIDS {
                guard.clear();
            }
            guard.insert(sid.to_string(), exp_ms);
        }
    }

    pub(crate) fn verify_csrf(
        &self,
        headers: &HeaderMap,
        claims: &SessionClaims,
    ) -> Result<(), ApiError> {
        if let Some(hdr) = headers.get("x-csrf-token").and_then(|v| v.to_str().ok()) {
            if ct_eq(hdr.trim().as_bytes(), claims.csrf.as_bytes()) {
                return Ok(());
            }
            return Err(ApiError::forbidden(
                "csrf_mismatch",
                "invalid X-CSRF-Token header",
            ));
        }
        if let Some(sec_fetch) = headers.get("sec-fetch-site").and_then(|v| v.to_str().ok())
            && sec_fetch.eq_ignore_ascii_case("same-origin")
        {
            return Ok(());
        }
        if let Some(origin) = headers.get(ORIGIN).and_then(|v| v.to_str().ok())
            && let Ok(parsed) = Url::parse(origin)
            && let Some(origin_host) = parsed.host_str().map(str::to_ascii_lowercase)
        {
            let host_matches = headers
                .get(HOST)
                .and_then(|v| v.to_str().ok())
                .and_then(|h| h.split(':').next())
                .is_some_and(|h| h.eq_ignore_ascii_case(&origin_host));
            if host_matches || self.public_host.as_deref() == Some(origin_host.as_str()) {
                return Ok(());
            }
        }
        Err(ApiError::forbidden(
            "csrf_required",
            "mutating session request requires matching X-CSRF-Token or same-origin header",
        ))
    }
}
