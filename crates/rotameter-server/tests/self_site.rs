#![forbid(unsafe_code)]
//! Integration tests for idempotent local `self_site` bootstrap and `/v1/` loop guard.

mod common;

use std::sync::Arc;
use std::time::Duration;

use axum::http::{Method, StatusCode};
use rotameter_server::{AppState, ServerConfig, build_router};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::json;

use common::{TestStore, as_json, request};

#[tokio::test]
async fn self_site_bootstrap_is_idempotent_and_guards_v1_paths() {
    let admin = "rm_adm_selfsite_test_token_12345";
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("self.db");
    let cfg = ServerConfig {
        bootstrap_admin_key: Some(admin.to_string()),
        public_url: Some("https://analytics.example.com".to_string()),
        self_site_enabled: true,
        ..ServerConfig::default()
    };

    let s1 = Arc::new(TestStore(
        SqliteStore::open(&db, SqliteConfig::default(), 1_700_000_000_000).unwrap(),
    ));
    let st1 = AppState::new(s1, &cfg).unwrap();
    let app1 = build_router(st1.clone());
    let (_, list1) = request(
        &app1,
        Method::GET,
        "/v1/admin/sites",
        Some(admin),
        &[],
        None,
    )
    .await;
    let arr1 = as_json(&list1);
    assert_eq!(arr1.as_array().unwrap().len(), 1);
    assert_eq!(arr1[0]["slug"].as_str(), Some("analytics.example.com"));
    assert_eq!(arr1[0]["is_self_site"].as_bool(), Some(true));
    let self_id = arr1[0]["site_id"].as_str().unwrap().to_string();

    let (st, loop_b) = request(
        &app1,
        Method::POST,
        "/v1/events",
        Some(admin),
        &[("user-agent", "Mozilla/5.0 Chrome/131.0")],
        Some(&json!({ "site_id": self_id, "url": "https://analytics.example.com/v1/query" })),
    )
    .await;
    assert_eq!(
        (st, as_json(&loop_b)["accepted"].as_u64()),
        (StatusCode::ACCEPTED, Some(0))
    );
    st1.ingest.shutdown(Duration::from_secs(2)).await;

    let s2 = Arc::new(TestStore(
        SqliteStore::open(&db, SqliteConfig::default(), 1_700_000_000_000).unwrap(),
    ));
    let st2 = AppState::new(s2, &cfg).unwrap();
    let app2 = build_router(st2.clone());
    let (_, list2) = request(
        &app2,
        Method::GET,
        "/v1/admin/sites",
        Some(admin),
        &[],
        None,
    )
    .await;
    assert_eq!(as_json(&list2).as_array().unwrap().len(), 1);
    assert_eq!(
        as_json(&list2)[0]["site_id"].as_str(),
        Some(self_id.as_str())
    );
    st2.ingest.shutdown(Duration::from_secs(2)).await;
}
