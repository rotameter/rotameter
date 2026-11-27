#![forbid(unsafe_code)]
//! Integration tests for `/v1/capabilities`, local admin session auth, CSRF, and cookieless collection.

mod common;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use rotameter_server::{
    AppState, OperatorAuthConfig, ServerConfig, ServerRole, ServerStores, UiConfig, UiMode,
    build_router,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common::{TestStore, as_json, request};

const ADMIN_KEY: &str = "rm_adm_0123456789abcdef0123456789abcdef";

async fn build_test_app(mut cfg: ServerConfig) -> Router {
    let Ok(sqlite) = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000) else {
        std::process::abort()
    };
    cfg.bootstrap_admin_key = Some(ADMIN_KEY.to_string());
    let store = TestStore::new(sqlite);
    let Ok(state) = AppState::new(ServerStores::unified(Arc::clone(&store)), &cfg).await else {
        std::process::abort()
    };
    build_router(state)
}

async fn send_raw(
    app: &Router,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
    body: Option<&Value>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(uri);
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let built = match body {
        Some(j) => b
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(j.to_string())),
        None => b.body(Body::empty()),
    };
    let Ok(req) = built else {
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
async fn capabilities_reflect_role_and_features() {
    let ingest_app = build_test_app(ServerConfig {
        role: ServerRole::Ingest,
        ..ServerConfig::default()
    })
    .await;
    let (st, body) = request(
        &ingest_app,
        Method::GET,
        "/v1/capabilities",
        None,
        &[],
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let json = as_json(&body);
    assert_eq!(json["topology"], "single");
    assert_eq!(json["node_role"], "ingest");
    assert_eq!(json["features"]["ui"], false);
    assert_eq!(json["auth_methods"], json!(["bearer"]));

    let dir = tempfile::tempdir().ok();
    let dir_path = dir.as_ref().map(|d| d.path().to_path_buf());
    let full_app = build_test_app(ServerConfig {
        role: ServerRole::All,
        ui: UiConfig {
            mode: UiMode::On,
            dir: dir_path,
        },
        operator_auth: OperatorAuthConfig {
            admin_password: Some("hunter2".to_string()),
            cf_access_trusted: true,
            ..OperatorAuthConfig::default()
        },
        ..ServerConfig::default()
    })
    .await;
    let (st2, body2) = request(&full_app, Method::GET, "/v1/capabilities", None, &[], None).await;
    assert_eq!(st2, StatusCode::OK);
    let j2 = as_json(&body2);
    assert_eq!(j2["features"]["ui"], true);
    assert_eq!(
        j2["auth_methods"],
        json!(["session", "bearer", "cf_access"])
    );
}

#[tokio::test]
async fn local_admin_session_csrf_logout_and_cookieless_collection() {
    let app = build_test_app(ServerConfig {
        public_url: Some("https://analytics.example.com".to_string()),
        operator_auth: OperatorAuthConfig {
            admin_username: "ops".to_string(),
            admin_password: Some("s3cret!".to_string()),
            ..OperatorAuthConfig::default()
        },
        ..ServerConfig::default()
    })
    .await;

    let (bad_st, _, _) = send_raw(
        &app,
        Method::POST,
        "/v1/auth/login",
        &[],
        Some(&json!({ "username": "ops", "password": "wrong" })),
    )
    .await;
    assert_eq!(bad_st, StatusCode::UNAUTHORIZED);

    let (ok_st, login_hdrs, login_body) = send_raw(
        &app,
        Method::POST,
        "/v1/auth/login",
        &[],
        Some(&json!({ "username": "ops", "password": "s3cret!" })),
    )
    .await;
    assert_eq!(ok_st, StatusCode::OK);
    let login_json = as_json(&login_body);
    let csrf = login_json["csrf_token"].as_str().unwrap_or_default();
    let sess_cookie = cookie_pair(&login_hdrs, "rotameter_session").unwrap_or_default();
    assert_ne!(sess_cookie, "");
    let set_cookies: Vec<String> = login_hdrs
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(ToString::to_string))
        .collect();
    assert!(
        set_cookies
            .iter()
            .any(|c| c.contains("HttpOnly") && c.contains("SameSite=Lax") && c.contains("Secure"))
    );

    let (me_st, _, me_body) = send_raw(
        &app,
        Method::GET,
        "/v1/auth/me",
        &[("cookie", &sess_cookie)],
        None,
    )
    .await;
    assert_eq!(me_st, StatusCode::OK);
    assert_eq!(as_json(&me_body)["subject"], "ops");

    let (no_csrf_st, _, _) = send_raw(
        &app,
        Method::POST,
        "/v1/admin/sites",
        &[("cookie", &sess_cookie)],
        Some(&json!({ "slug": "demo.dev" })),
    )
    .await;
    assert_eq!(no_csrf_st, StatusCode::FORBIDDEN);

    let (create_st, _, _) = send_raw(
        &app,
        Method::POST,
        "/v1/admin/sites",
        &[("cookie", &sess_cookie), ("x-csrf-token", csrf)],
        Some(&json!({ "slug": "demo.dev" })),
    )
    .await;
    assert_eq!(create_st, StatusCode::CREATED);

    let (col_st, col_hdrs, _) = send_raw(
        &app,
        Method::GET,
        "/v1/collect.js",
        &[("cookie", &sess_cookie)],
        None,
    )
    .await;
    assert_eq!(col_st, StatusCode::OK);
    assert!(col_hdrs.get(header::SET_COOKIE).is_none());

    let (out_st, _, _) = send_raw(
        &app,
        Method::POST,
        "/v1/auth/logout",
        &[("cookie", &sess_cookie), ("x-csrf-token", csrf)],
        None,
    )
    .await;
    assert_eq!(out_st, StatusCode::OK);
    let (after_st, _, _) = send_raw(
        &app,
        Method::GET,
        "/v1/auth/me",
        &[("cookie", &sess_cookie)],
        None,
    )
    .await;
    assert_eq!(after_st, StatusCode::UNAUTHORIZED);
}
