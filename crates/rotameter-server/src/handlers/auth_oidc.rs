#![forbid(unsafe_code)]
//! OIDC Authorization Code + PKCE endpoints (`/v1/auth/oidc/start`, `/v1/auth/oidc/callback`).

use axum::extract::{Query, State};
use axum::http::HeaderValue;
use axum::http::header::LOCATION;
use axum::response::{IntoResponse, Redirect, Response};
use rotameter_core::types::KeyScope;
use serde::Deserialize;

use crate::error::ApiError;
use crate::handlers::auth::append_cookies;
use crate::state::{AppState, now_ms};

#[derive(Deserialize)]
pub(crate) struct OidcStartQuery {
    #[serde(default)]
    return_to: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct OidcCallbackQuery {
    code: String,
    state: String,
}

fn sanitize_return_to(raw: Option<&str>) -> String {
    match raw.map(str::trim) {
        Some(path) if path.starts_with('/') && !path.starts_with("//") => path.to_string(),
        _ => "/".to_string(),
    }
}

pub(crate) async fn get_oidc_start(
    State(state): State<AppState>,
    Query(query): Query<OidcStartQuery>,
) -> Result<Response, ApiError> {
    let Some(oidc) = state.operator_auth.oidc.as_ref() else {
        return Err(ApiError::not_found(
            "oidc_disabled",
            "OIDC authentication is not configured on this server",
        ));
    };
    let return_to = sanitize_return_to(query.return_to.as_deref());
    let auth_url = oidc.start_auth(now_ms(), return_to).await?;
    Ok(Redirect::temporary(&auth_url).into_response())
}

pub(crate) async fn get_oidc_callback(
    State(state): State<AppState>,
    Query(query): Query<OidcCallbackQuery>,
) -> Result<Response, ApiError> {
    let Some(oidc) = state.operator_auth.oidc.as_ref() else {
        return Err(ApiError::not_found(
            "oidc_disabled",
            "OIDC authentication is not configured on this server",
        ));
    };
    let now = now_ms();
    let (subject, return_to) = oidc.finish_callback(&query.code, &query.state, now).await?;
    let (_, cookies) = state.operator_auth.issue_session(
        subject,
        state.default_org_id,
        KeyScope::Admin,
        None,
        "oidc",
        now,
    )?;
    let mut resp = Redirect::temporary(&return_to).into_response();
    if let Ok(loc) = HeaderValue::from_str(&return_to) {
        resp.headers_mut().insert(LOCATION, loc);
    }
    append_cookies(&mut resp, &cookies);
    Ok(resp)
}
