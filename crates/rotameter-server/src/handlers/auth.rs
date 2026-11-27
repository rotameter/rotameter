#![forbid(unsafe_code)]
//! Operator session auth endpoints (`/v1/auth/login`, `/v1/auth/logout`, `/v1/auth/me`).

use axum::Json;
use axum::extract::State;
use axum::http::header::SET_COOKIE;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use rotameter_core::privacy::compute_api_key_hash;
use rotameter_core::types::KeyScope;
use serde::{Deserialize, Serialize};

use crate::auth::extract_operator_auth;
use crate::auth_codec::extract_cookie;
use crate::auth_session::SESSION_COOKIE_NAME;
use crate::error::ApiError;
use crate::state::{AppState, now_ms};

#[derive(Deserialize)]
pub(crate) struct LoginRequest {
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
    #[serde(default)]
    token: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct AuthMeResponse {
    authenticated: bool,
    method: &'static str,
    subject: String,
    scope: KeyScope,
    org_id: rotameter_core::id::OrgId,
    #[serde(skip_serializing_if = "Option::is_none")]
    site_id: Option<rotameter_core::id::SiteId>,
}

pub(crate) async fn post_login(
    State(state): State<AppState>,
    Json(req): Json<LoginRequest>,
) -> Result<Response, ApiError> {
    let (sub, org_id, scope, site_id) = if let Some(raw_tok) = req
        .token
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let hash = compute_api_key_hash(raw_tok);
        let rec = state
            .control_plane
            .find_active_key_by_hash(&hash)
            .await
            .map_err(ApiError::from)?
            .ok_or_else(|| ApiError::unauthorized("invalid or revoked API key"))?;
        if !matches!(rec.scope, KeyScope::Admin | KeyScope::Read) {
            return Err(ApiError::forbidden(
                "insufficient_scope",
                "Web UI login requires a `read` or `admin` API key",
            ));
        }
        (rec.key_prefix, rec.org_id, rec.scope, rec.site_id)
    } else if let Some(password) = req.password.as_deref() {
        let username = req.username.as_deref().unwrap_or("admin").trim();
        if !state
            .operator_auth
            .verify_admin_credentials(username, password)
        {
            return Err(ApiError::unauthorized("invalid operator credentials"));
        }
        (
            username.to_string(),
            state.default_org_id,
            KeyScope::Admin,
            None,
        )
    } else {
        return Err(ApiError::bad_request(
            "missing_credentials",
            "provide `username`/`password` or `token`",
        ));
    };

    let (csrf_token, cookies) = state.operator_auth.issue_session(
        sub.clone(),
        org_id,
        scope,
        site_id,
        "session",
        now_ms(),
    )?;
    let mut body = serde_json::json!({
        "authenticated": true,
        "method": "session",
        "subject": sub,
        "scope": scope,
        "org_id": org_id,
        "csrf_token": csrf_token,
    });
    if let Some(sid) = site_id
        && let Some(map) = body.as_object_mut()
    {
        map.insert("site_id".to_string(), serde_json::json!(sid));
    }
    let mut resp = (StatusCode::OK, Json(body)).into_response();
    append_cookies(&mut resp, &cookies);
    Ok(resp)
}

pub(crate) async fn post_logout(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let now = now_ms();
    if let Some(raw_cookie) = extract_cookie(&headers, SESSION_COOKIE_NAME)
        && let Some(claims) = state.operator_auth.verify_session_cookie(&raw_cookie, now)
    {
        state.operator_auth.verify_csrf(&headers, &claims)?;
        state
            .operator_auth
            .revoke_session(&claims.sid, claims.exp_ms, now);
    }
    let cookies = state.operator_auth.clear_cookie_headers();
    let mut resp = (
        StatusCode::OK,
        Json(serde_json::json!({ "authenticated": false })),
    )
        .into_response();
    append_cookies(&mut resp, &cookies);
    Ok(resp)
}

pub(crate) async fn get_me(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<AuthMeResponse>, ApiError> {
    let ident = extract_operator_auth(&state, &headers, false)
        .await?
        .ok_or_else(|| ApiError::unauthorized("not authenticated"))?;
    Ok(Json(AuthMeResponse {
        authenticated: true,
        method: ident.method,
        subject: ident.subject,
        scope: ident.auth.scope,
        org_id: ident.auth.org_id,
        site_id: ident.auth.site_id,
    }))
}

pub(crate) fn append_cookies(resp: &mut Response, cookies: &[String]) {
    for raw in cookies {
        if let Ok(val) = HeaderValue::from_str(raw) {
            resp.headers_mut().append(SET_COOKIE, val);
        }
    }
}
