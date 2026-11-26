#![forbid(unsafe_code)]
//! Handler contract tests for `POST /v1/admin/backup` in `rotameter-server`.

mod common;

use axum::http::{Method, StatusCode};
use common::{TestStore, as_json, request};
use rotameter_server::{AppState, ServerConfig, ServerStores, build_router};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

const ADMIN_KEY: &str = "rm_adm_backup_contract_token";

#[tokio::test]
async fn post_backup_requires_admin_and_returns_501_without_exporter() {
    let dir = tempfile::tempdir().unwrap();
    let sqlite = SqliteStore::open(
        dir.path().join("ctrl.db"),
        SqliteConfig::default(),
        1_700_000_000_000,
    )
    .unwrap();
    let store = TestStore::new(sqlite);
    let state = AppState::new(
        ServerStores::unified(store),
        &ServerConfig {
            bootstrap_admin_key: Some(ADMIN_KEY.to_string()),
            self_site_enabled: false,
            ..ServerConfig::default()
        },
    )
    .await
    .unwrap();
    let app = build_router(state);

    let (unauth_status, _) = request(&app, Method::POST, "/v1/admin/backup", None, &[], None).await;
    assert_eq!(unauth_status, StatusCode::UNAUTHORIZED);

    let (status, body) = request(
        &app,
        Method::POST,
        "/v1/admin/backup",
        Some(ADMIN_KEY),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
    assert_eq!(as_json(&body)["error"]["code"], "not_implemented");
}
