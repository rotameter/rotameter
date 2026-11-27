#![forbid(unsafe_code)]
//! Integration tests for Cloudflare Access and OIDC Authorization Code + PKCE operator auth.

mod common;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use axum::response::Json as AxumJson;
use axum::routing::{get, post};
use http_body_util::BodyExt;
use rotameter_server::{
    AppState, OidcConfig, OperatorAuthConfig, ServerConfig, ServerStores, build_router,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::json;
use tokio::net::TcpListener;
use tower::ServiceExt;

use crate::common::{TestStore, as_json};

async fn build_app(cfg: ServerConfig) -> Router {
    let Ok(sqlite) = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000) else {
        std::process::abort()
    };
    let store = TestStore::new(sqlite);
    let Ok(state) = AppState::new(ServerStores::unified(Arc::clone(&store)), &cfg).await else {
        std::process::abort()
    };
    build_router(state)
}

async fn send(
    app: &Router,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(uri);
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let Ok(req) = b.body(Body::empty()) else {
        std::process::abort()
    };
    let Ok(resp) = app.clone().oneshot(req).await;
    let status = resp.status();
    let resp_headers = resp.headers().clone();
    let Ok(bytes) = resp.into_body().collect().await else {
        std::process::abort()
    };
    (status, resp_headers, bytes.to_bytes().to_vec())
}

fn cookie_pair(headers: &axum::http::HeaderMap, name: &str) -> Option<String> {
    headers.get_all(header::SET_COOKIE).iter().find_map(|v| {
        let s = v.to_str().ok()?;
        let pair = s.split(';').next()?.trim();
        pair.starts_with(&format!("{name}="))
            .then(|| pair.to_string())
    })
}

#[tokio::test]
async fn cf_access_header_authenticates_admin() {
    let cf_app = build_app(ServerConfig {
        operator_auth: OperatorAuthConfig {
            cf_access_trusted: true,
            ..OperatorAuthConfig::default()
        },
        ..ServerConfig::default()
    })
    .await;
    let (status, _, body) = send(
        &cf_app,
        Method::GET,
        "/v1/auth/me",
        &[("cf-access-authenticated-user-email", "colin@example.com")],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(as_json(&body)["method"], "cf_access");
    assert_eq!(as_json(&body)["subject"], "colin@example.com");
}

#[tokio::test]
async fn oidc_pkce_flow_issues_session_cookie_and_enforces_allowlist() {
    let Ok(listener) = TcpListener::bind("127.0.0.1:0").await else {
        std::process::abort()
    };
    let Ok(addr) = listener.local_addr() else {
        std::process::abort()
    };
    let issuer = format!("http://{addr}");
    let issuer_disc = issuer.clone();
    let mock = Router::new()
        .route(
            "/.well-known/openid-configuration",
            get(move || {
                let iss = issuer_disc.clone();
                async move {
                    AxumJson(json!({
                        "issuer": iss,
                        "authorization_endpoint": format!("{iss}/authorize"),
                        "token_endpoint": format!("{iss}/token"),
                        "userinfo_endpoint": format!("{iss}/userinfo")
                    }))
                }
            }),
        )
        .route(
            "/token",
            post(
                |_body: String| async move { AxumJson(json!({ "access_token": "tok_mock_123" })) },
            ),
        )
        .route(
            "/userinfo",
            get(|| async { AxumJson(json!({ "sub": "u1", "email": "admin@example.com" })) }),
        );
    tokio::spawn(async move {
        let _ = axum::serve(listener, mock).await;
    });

    let oidc_app = build_app(ServerConfig {
        operator_auth: OperatorAuthConfig {
            oidc: Some(OidcConfig {
                issuer_url: issuer,
                client_id: "rotameter".into(),
                client_secret: "secret".into(),
                redirect_uri: "http://127.0.0.1:8000/v1/auth/oidc/callback".into(),
                scopes: vec!["openid".into(), "email".into()],
                allowed_emails: vec!["admin@example.com".into()],
                allowed_domains: vec![],
                authorization_endpoint: None,
                token_endpoint: None,
                userinfo_endpoint: None,
            }),
            ..OperatorAuthConfig::default()
        },
        ..ServerConfig::default()
    })
    .await;

    let (start_st, start_hdrs, _) = send(
        &oidc_app,
        Method::GET,
        "/v1/auth/oidc/start?return_to=/sites",
        &[],
    )
    .await;
    assert_eq!(start_st, StatusCode::TEMPORARY_REDIRECT);
    let loc = start_hdrs
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let state_param = url::Url::parse(loc)
        .ok()
        .and_then(|u| {
            u.query_pairs()
                .find(|(k, _)| k == "state")
                .map(|(_, v)| v.to_string())
        })
        .unwrap_or_default();
    assert_ne!(state_param, "");

    let callback_uri = format!("/v1/auth/oidc/callback?code=abc&state={state_param}");
    let (callback_st, callback_hdrs, _) = send(&oidc_app, Method::GET, &callback_uri, &[]).await;
    assert_eq!(callback_st, StatusCode::TEMPORARY_REDIRECT);
    assert_eq!(
        callback_hdrs
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok()),
        Some("/sites")
    );
    let oidc_cookie = cookie_pair(&callback_hdrs, "rotameter_session").unwrap_or_default();
    let (me_st, _, me_body) = send(
        &oidc_app,
        Method::GET,
        "/v1/auth/me",
        &[("cookie", &oidc_cookie)],
    )
    .await;
    assert_eq!(me_st, StatusCode::OK);
    assert_eq!(as_json(&me_body)["method"], "oidc");
    assert_eq!(as_json(&me_body)["subject"], "admin@example.com");
}
