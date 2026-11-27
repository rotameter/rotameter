#![forbid(unsafe_code)]
//! Integration test verifying `rm_rd_` operator cookie sessions preserve `site_id` scoping.

mod common;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use rotameter_server::{AppState, ServerConfig, ServerStores, build_router};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::{Value, json};
use tower::ServiceExt;

use crate::common::{TestStore, as_json, request};

const ADMIN_KEY: &str = "rm_adm_0123456789abcdef0123456789abcdef";

async fn build_app() -> Router {
    let Ok(sqlite) = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000) else {
        std::process::abort()
    };
    let store = TestStore::new(sqlite);
    let Ok(state) = AppState::new(
        ServerStores::unified(Arc::clone(&store)),
        &ServerConfig {
            bootstrap_admin_key: Some(ADMIN_KEY.to_string()),
            ..ServerConfig::default()
        },
    )
    .await
    else {
        std::process::abort()
    };
    build_router(state)
}

async fn send_json(
    app: &Router,
    method: Method,
    uri: &str,
    headers: &[(&str, &str)],
    payload: Option<&Value>,
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(uri);
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let built = match payload {
        Some(v) => b
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(v.to_string())),
        None => b.body(Body::empty()),
    };
    let Ok(req) = built else {
        std::process::abort()
    };
    let Ok(resp) = app.clone().oneshot(req).await;
    let status = resp.status();
    let hdrs = resp.headers().clone();
    let Ok(bytes) = resp.into_body().collect().await else {
        std::process::abort()
    };
    (status, hdrs, bytes.to_bytes().to_vec())
}

#[tokio::test]
async fn read_token_login_preserves_site_id_in_session_cookie() {
    let app = build_app().await;
    let (s1, b1) = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(ADMIN_KEY),
        &[],
        Some(&json!({ "slug": "ro-site.dev" })),
    )
    .await;
    assert_eq!(s1, StatusCode::CREATED);
    let site_id = as_json(&b1)["site_id"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let (s2, b2) = request(
        &app,
        Method::POST,
        "/v1/admin/keys",
        Some(ADMIN_KEY),
        &[],
        Some(&json!({ "scope": "read", "site": site_id, "label": "ui-ro" })),
    )
    .await;
    assert_eq!(s2, StatusCode::CREATED);
    let ro_tok = as_json(&b2)["token"]
        .as_str()
        .unwrap_or_default()
        .to_string();

    let (login_st, login_hdrs, login_bytes) = send_json(
        &app,
        Method::POST,
        "/v1/auth/login",
        &[],
        Some(&json!({ "token": ro_tok })),
    )
    .await;
    assert_eq!(login_st, StatusCode::OK);
    let login_json = as_json(&login_bytes);
    assert_eq!(login_json["scope"], "read");
    assert_eq!(login_json["site_id"], site_id);
    let csrf = login_json["csrf_token"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    let cookie = login_hdrs
        .get_all(header::SET_COOKIE)
        .iter()
        .find_map(|v| {
            let s = v.to_str().ok()?;
            let pair = s.split(';').next()?.trim();
            pair.starts_with("rotameter_session=")
                .then(|| pair.to_string())
        })
        .unwrap_or_default();

    let (me_st, _, me_bytes) = send_json(
        &app,
        Method::GET,
        "/v1/auth/me",
        &[("cookie", &cookie)],
        None,
    )
    .await;
    assert_eq!(me_st, StatusCode::OK);
    assert_eq!(as_json(&me_bytes)["site_id"], site_id);

    let (q_st, _, _) = send_json(
        &app,
        Method::POST,
        "/v1/query",
        &[("cookie", &cookie), ("x-csrf-token", &csrf)],
        Some(&json!({ "site_id": "ro-site.dev", "range": { "preset": "7d" }, "metrics": ["visitors"] })),
    )
    .await;
    assert_eq!(q_st, StatusCode::OK);

    let (adm_st, _, _) = send_json(
        &app,
        Method::GET,
        "/v1/admin/sites",
        &[("cookie", &cookie)],
        None,
    )
    .await;
    assert_eq!(adm_st, StatusCode::FORBIDDEN);
}
