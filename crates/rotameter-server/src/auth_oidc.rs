#![forbid(unsafe_code)]
//! `OpenID` Connect (Authorization Code + PKCE `S256`) state and token exchange manager.

use std::collections::HashMap;
use std::sync::Mutex;

use hmac_sha256::Hash;
use serde_json::Value;
use url::Url;

use crate::auth_codec::{b64url_encode, hex_encode};
use crate::auth_config::OidcConfig;
use crate::auth_oidc_http::{DiscoveryDoc, exchange_code, fetch_discovery, resolve_claims};
use crate::error::ApiError;
use crate::state::fill_random;

const MAX_PENDING_FLOWS: usize = 1_024;
const FLOW_TTL_MS: i64 = 600_000;

#[derive(Debug, Clone)]
struct PendingOidcFlow {
    nonce: String,
    code_verifier: String,
    return_to: String,
    expires_at_ms: i64,
}

pub(crate) struct OidcManager {
    config: OidcConfig,
    http: reqwest::Client,
    discovery: tokio::sync::Mutex<Option<DiscoveryDoc>>,
    pending: Mutex<HashMap<String, PendingOidcFlow>>,
}

impl OidcManager {
    pub(crate) fn new(config: OidcConfig) -> Self {
        Self {
            config,
            http: reqwest::Client::new(),
            discovery: tokio::sync::Mutex::new(None),
            pending: Mutex::new(HashMap::new()),
        }
    }

    async fn resolve_endpoints(&self) -> Result<DiscoveryDoc, ApiError> {
        if let (Some(auth), Some(tok)) = (
            &self.config.authorization_endpoint,
            &self.config.token_endpoint,
        ) {
            return Ok(DiscoveryDoc {
                authorization_endpoint: auth.clone(),
                token_endpoint: tok.clone(),
                userinfo_endpoint: self.config.userinfo_endpoint.clone(),
            });
        }
        let mut cached = self.discovery.lock().await;
        if let Some(doc) = cached.as_ref() {
            return Ok(doc.clone());
        }
        let doc = fetch_discovery(&self.http, &self.config.issuer_url).await?;
        *cached = Some(doc.clone());
        Ok(doc)
    }

    pub(crate) async fn start_auth(
        &self,
        now_ms: i64,
        return_to: String,
    ) -> Result<String, ApiError> {
        let doc = self.resolve_endpoints().await?;
        let state = hex_encode(&fill_random::<16>().map_err(ApiError::from)?);
        let nonce = hex_encode(&fill_random::<16>().map_err(ApiError::from)?);
        let verifier = b64url_encode(&fill_random::<32>().map_err(ApiError::from)?);
        let challenge = b64url_encode(&Hash::hash(verifier.as_bytes()));
        {
            let mut guard = self
                .pending
                .lock()
                .map_err(|_| ApiError::internal("oidc pending lock poisoned"))?;
            guard.retain(|_, v| v.expires_at_ms > now_ms);
            if guard.len() >= MAX_PENDING_FLOWS {
                guard.clear();
            }
            guard.insert(
                state.clone(),
                PendingOidcFlow {
                    nonce: nonce.clone(),
                    code_verifier: verifier,
                    return_to,
                    expires_at_ms: now_ms.saturating_add(FLOW_TTL_MS),
                },
            );
        }
        let scopes = if self.config.scopes.is_empty() {
            "openid email profile".to_string()
        } else {
            self.config.scopes.join(" ")
        };
        let mut url = Url::parse(&doc.authorization_endpoint)
            .map_err(|e| ApiError::internal(format!("invalid OIDC auth endpoint: {e}")))?;
        url.query_pairs_mut()
            .append_pair("response_type", "code")
            .append_pair("client_id", &self.config.client_id)
            .append_pair("redirect_uri", &self.config.redirect_uri)
            .append_pair("scope", &scopes)
            .append_pair("state", &state)
            .append_pair("nonce", &nonce)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
        Ok(url.into())
    }

    pub(crate) async fn finish_callback(
        &self,
        code: &str,
        state: &str,
        now_ms: i64,
    ) -> Result<(String, String), ApiError> {
        let flow = self.take_flow(state, now_ms)?;
        let doc = self.resolve_endpoints().await?;
        let tok = exchange_code(
            &self.http,
            &self.config,
            &doc.token_endpoint,
            code,
            &flow.code_verifier,
        )
        .await?;
        let claims = resolve_claims(&self.http, &doc, &tok, &flow.nonce).await?;
        let email = claims
            .get("email")
            .and_then(Value::as_str)
            .map(str::to_ascii_lowercase);
        let subject = email
            .clone()
            .or_else(|| {
                claims
                    .get("sub")
                    .and_then(Value::as_str)
                    .map(ToString::to_string)
            })
            .ok_or_else(|| ApiError::unauthorized("OIDC identity missing email/sub"))?;
        self.check_allowlists(email.as_deref())?;
        Ok((subject, flow.return_to))
    }

    fn take_flow(&self, state: &str, now_ms: i64) -> Result<PendingOidcFlow, ApiError> {
        let flow = {
            let mut guard = self
                .pending
                .lock()
                .map_err(|_| ApiError::internal("oidc pending lock poisoned"))?;
            guard.remove(state)
        }
        .ok_or_else(|| ApiError::unauthorized("invalid or expired OIDC state"))?;
        if flow.expires_at_ms <= now_ms {
            return Err(ApiError::unauthorized("expired OIDC authorization state"));
        }
        Ok(flow)
    }

    fn check_allowlists(&self, email: Option<&str>) -> Result<(), ApiError> {
        if self.config.allowed_emails.is_empty() && self.config.allowed_domains.is_empty() {
            return Ok(());
        }
        let Some(addr) = email else {
            return Err(ApiError::forbidden(
                "oidc_forbidden",
                "OIDC user email required",
            ));
        };
        if self
            .config
            .allowed_emails
            .iter()
            .any(|e| e.eq_ignore_ascii_case(addr))
        {
            return Ok(());
        }
        if let Some((_, domain)) = addr.split_once('@')
            && self
                .config
                .allowed_domains
                .iter()
                .any(|d| d.eq_ignore_ascii_case(domain))
        {
            return Ok(());
        }
        Err(ApiError::forbidden(
            "oidc_forbidden",
            "OIDC user is not in allowed_emails or allowed_domains",
        ))
    }
}
