#![forbid(unsafe_code)]
//! Integration tests for `ROTAMETER_BOOTSTRAP_ADMIN_KEY`, `/v1/health`, and `/v1/admin/sites/*`.

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use rotameter_server::{AppState, ServerConfig, ServerStores, build_router};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::json;

use common::{TestStore, as_json, request};

#[tokio::test]
async fn bootstrap_admin_is_idempotent_and_sites_crud_works() {
    let admin = "rm_adm_bootstrap_test_token_123456";
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("srv.db");
    let cfg = ServerConfig {
        bootstrap_admin_key: Some(admin.to_string()),
        ..ServerConfig::default()
    };

    let s1 =
        TestStore::new(SqliteStore::open(&db, SqliteConfig::default(), 1_700_000_000_000).unwrap());
    AppState::new(ServerStores::unified(s1), &cfg)
        .await
        .unwrap()
        .ingest
        .shutdown(Duration::from_secs(1))
        .await;

    let s2 =
        TestStore::new(SqliteStore::open(&db, SqliteConfig::default(), 1_700_000_000_000).unwrap());
    let state = AppState::new(ServerStores::unified(s2), &cfg)
        .await
        .unwrap();
    let app = build_router(state.clone());

    let (st, hb) = request(&app, Method::GET, "/v1/health", None, &[], None).await;
    assert_eq!(
        (st, as_json(&hb)["status"].as_str()),
        (StatusCode::OK, Some("ok"))
    );
    assert_eq!(
        request(&app, Method::GET, "/v1/admin/sites", None, &[], None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    let (st, sb) = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "colinwolff.dev" })),
    )
    .await;
    assert_eq!(
        (st, as_json(&sb)["slug"].as_str()),
        (StatusCode::CREATED, Some("colinwolff.dev"))
    );

    let (st, ub) = request(
        &app,
        Method::PUT,
        "/v1/admin/sites/colinwolff.dev",
        Some(admin),
        &[],
        Some(&json!({ "display_name": "CW" })),
    )
    .await;
    assert_eq!(
        (st, as_json(&ub)["display_name"].as_str()),
        (StatusCode::OK, Some("CW"))
    );

    assert_eq!(
        request(
            &app,
            Method::DELETE,
            "/v1/admin/sites/colinwolff.dev",
            Some(admin),
            &[],
            None
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        request(
            &app,
            Method::DELETE,
            "/v1/admin/sites/colinwolff.dev?confirm=true",
            Some(admin),
            &[],
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    state.ingest.shutdown(Duration::from_secs(2)).await;
}
