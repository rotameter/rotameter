#![forbid(unsafe_code)]
//! HTTP discovery, token exchange, and userinfo helpers for `OidcManager`.

use serde::Deserialize;
use serde_json::Value;

use crate::auth_codec::decode_jwt_payload;
use crate::auth_config::OidcConfig;
use crate::error::ApiError;

#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct DiscoveryDoc {
    pub(crate) authorization_endpoint: String,
    pub(crate) token_endpoint: String,
    #[serde(default)]
    pub(crate) userinfo_endpoint: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TokenResponse {
    #[serde(default)]
    pub(crate) access_token: Option<String>,
    #[serde(default)]
    pub(crate) id_token: Option<String>,
}

pub(crate) async fn fetch_discovery(
    http: &reqwest::Client,
    issuer_url: &str,
) -> Result<DiscoveryDoc, ApiError> {
    let base = issuer_url.trim_end_matches('/');
    let url = format!("{base}/.well-known/openid-configuration");
    let resp = http
        .get(&url)
        .send()
        .await
        .map_err(|e| ApiError::service_unavailable("oidc_discovery_failed", e.to_string()))?;
    if !resp.status().is_success() {
        return Err(ApiError::service_unavailable(
            "oidc_discovery_failed",
            format!("OIDC discovery returned {}", resp.status()),
        ));
    }
    resp.json()
        .await
        .map_err(|e| ApiError::service_unavailable("oidc_discovery_invalid", e.to_string()))
}

pub(crate) async fn exchange_code(
    http: &reqwest::Client,
    cfg: &OidcConfig,
    token_endpoint: &str,
    code: &str,
    code_verifier: &str,
) -> Result<TokenResponse, ApiError> {
    let resp = http
        .post(token_endpoint)
        .form(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", cfg.redirect_uri.as_str()),
            ("client_id", cfg.client_id.as_str()),
            ("client_secret", cfg.client_secret.as_str()),
            ("code_verifier", code_verifier),
        ])
        .send()
        .await
        .map_err(|e| ApiError::service_unavailable("oidc_token_failed", e.to_string()))?;
    if !resp.status().is_success() {
        return Err(ApiError::unauthorized("OIDC token exchange failed"));
    }
    resp.json()
        .await
        .map_err(|e| ApiError::unauthorized(format!("invalid OIDC token JSON: {e}")))
}

pub(crate) async fn resolve_claims(
    http: &reqwest::Client,
    doc: &DiscoveryDoc,
    tok: &TokenResponse,
    expected_nonce: &str,
) -> Result<Value, ApiError> {
    if let Some(id_tok) = tok.id_token.as_deref() {
        let jwt_claims = decode_jwt_payload(id_tok)?;
        if let Some(nonce) = jwt_claims.get("nonce").and_then(Value::as_str)
            && nonce != expected_nonce
        {
            return Err(ApiError::unauthorized("OIDC nonce mismatch"));
        }
        return Ok(jwt_claims);
    }
    if let (Some(ui_url), Some(access)) = (
        doc.userinfo_endpoint.as_deref(),
        tok.access_token.as_deref(),
    ) {
        return fetch_userinfo(http, ui_url, access).await;
    }
    Err(ApiError::unauthorized(
        "OIDC response missing id_token and userinfo",
    ))
}

async fn fetch_userinfo(
    http: &reqwest::Client,
    url: &str,
    access_token: &str,
) -> Result<Value, ApiError> {
    let resp = http
        .get(url)
        .bearer_auth(access_token)
        .send()
        .await
        .map_err(|e| ApiError::service_unavailable("oidc_userinfo_failed", e.to_string()))?;
    if !resp.status().is_success() {
        return Err(ApiError::unauthorized("OIDC userinfo request failed"));
    }
    resp.json()
        .await
        .map_err(|e| ApiError::unauthorized(format!("invalid OIDC userinfo JSON: {e}")))
}
