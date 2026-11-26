#![forbid(unsafe_code)]
//! Integration tests for RM-32: DB-backed persistent salts, auto-pruning, and `--role` gating.

mod common;

use std::path::Path;
use std::sync::Arc;

use axum::Router;
use axum::http::{Method, StatusCode};
use common::{TestStore, as_json, request};
use rotameter_core::privacy::SaltWindow;
use rotameter_core::storage::ControlPlaneStore;
use rotameter_core::types::SaltRotation;
use rotameter_server::{
    AppState, SaltStoreMode, ServerConfig, ServerRole, ServerStores, build_router,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::{Value, json};

const ADMIN_KEY: &str = "rm_adm_multinode_test_token";

async fn file_node(
    db_path: &Path,
    role: ServerRole,
    salt_store: SaltStoreMode,
    self_site_enabled: bool,
) -> (AppState, Router, Arc<TestStore>) {
    let Ok(sqlite) = SqliteStore::open(db_path, SqliteConfig::default(), 1_700_000_000_000) else {
        std::process::abort()
    };
    let store = TestStore::new(sqlite);
    let Ok(state) = AppState::new(
        ServerStores::unified(store.clone()),
        &ServerConfig {
            role,
            salt_store,
            bootstrap_admin_key: Some(ADMIN_KEY.to_string()),
            self_site_enabled,
            ..ServerConfig::default()
        },
    )
    .await
    else {
        std::process::abort()
    };
    let router = build_router(state.clone());
    (state, router, store)
}

#[tokio::test]
async fn db_salt_deduplicates_across_nodes_and_memory_opts_out() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("shared.db");

    let (state1, app1, store1) =
        file_node(&db_path, ServerRole::All, SaltStoreMode::Db, false).await;
    let stale_win = SaltWindow {
        rotation: SaltRotation::Daily,
        window_id: 1,
        start_ms: 86_400_000,
    };
    let seeded = store1
        .get_or_create_salt(stale_win, [0x77; 32], 100_000)
        .await
        .unwrap();
    assert_eq!(seeded, [0x77; 32]);

    // Second node shares the DB: startup prunes stale windows past the 30m grace period.
    let (state2, app2, _) = file_node(&db_path, ServerRole::All, SaltStoreMode::Db, false).await;
    let after_prune = store1
        .get_or_create_salt(stale_win, [0x99; 32], 100_000)
        .await
        .unwrap();
    assert_eq!(after_prune, [0x99; 32]);

    let (status, site_body) = request(
        &app1,
        Method::POST,
        "/v1/admin/sites",
        Some(ADMIN_KEY),
        &[],
        Some(&json!({ "slug": "example.com", "display_name": "Example" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let site_id = as_json(&site_body)["site_id"].as_str().unwrap().to_string();

    let hit = json!({ "site_id": site_id, "event": "pageview", "url": "https://example.com/p" });
    let db_hdrs = [
        ("CF-Connecting-IP", "198.51.100.42"),
        ("User-Agent", "Mozilla/5.0 MultinodeTest"),
    ];
    for app in [&app1, &app2] {
        assert_eq!(
            request(app, Method::POST, "/v1/events", None, &db_hdrs, Some(&hit))
                .await
                .0,
            StatusCode::ACCEPTED
        );
    }
    state1.ingest.flush_now().await;
    state2.ingest.flush_now().await;

    let query = json!({
        "site_id": site_id,
        "metrics": ["pageviews", "visitors"],
        "range": { "preset": "24h" }
    });
    let (q_status, q_body) = request(
        &app1,
        Method::POST,
        "/v1/query",
        Some(ADMIN_KEY),
        &[],
        Some(&query),
    )
    .await;
    assert_eq!(q_status, StatusCode::OK);
    let db_totals = as_json(&q_body);
    assert_eq!(db_totals["summary"]["pageviews"], 2);
    assert_eq!(db_totals["summary"]["visitors"], 1);

    // Explicit `memory` opt-out produces distinct ephemeral salts per process/node.
    let (m1, m_app1, _) = file_node(&db_path, ServerRole::All, SaltStoreMode::Memory, false).await;
    let (m2, m_app2, _) = file_node(&db_path, ServerRole::All, SaltStoreMode::Memory, false).await;
    let mem_hdrs = [
        ("CF-Connecting-IP", "198.51.100.99"),
        ("User-Agent", "Mozilla/5.0 EphemeralTest"),
    ];
    for app in [&m_app1, &m_app2] {
        assert_eq!(
            request(app, Method::POST, "/v1/events", None, &mem_hdrs, Some(&hit))
                .await
                .0,
            StatusCode::ACCEPTED
        );
    }
    m1.ingest.flush_now().await;
    m2.ingest.flush_now().await;

    let (_, mem_query_body) = request(
        &m_app1,
        Method::POST,
        "/v1/query",
        Some(ADMIN_KEY),
        &[],
        Some(&query),
    )
    .await;
    let mem_totals = as_json(&mem_query_body);
    assert_eq!(mem_totals["summary"]["pageviews"], 4);
    assert_eq!(mem_totals["summary"]["visitors"], 3);
}

#[tokio::test]
async fn role_ingest_and_query_gate_routes_and_pipelines() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("roles.db");

    let (ing_state, ing_app, _) =
        file_node(&db_path, ServerRole::Ingest, SaltStoreMode::Db, true).await;
    assert!(ing_state.self_site_id().is_some());
    assert!(ing_state.ingest.capacity() > 0);

    let (h_status, h_body) = request(&ing_app, Method::GET, "/v1/health", None, &[], None).await;
    assert_eq!(h_status, StatusCode::OK);
    let h_json: Value = as_json(&h_body);
    assert_eq!(h_json["role"], "ingest");
    assert!(h_json["queue_capacity"].as_u64().unwrap() > 0);
    assert_eq!(
        request(&ing_app, Method::GET, "/v1/collect.js", None, &[], None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &ing_app,
            Method::POST,
            "/v1/query",
            Some(ADMIN_KEY),
            &[],
            Some(&json!({}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &ing_app,
            Method::GET,
            "/v1/admin/sites",
            Some(ADMIN_KEY),
            &[],
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );

    let (qry_state, qry_app, _) =
        file_node(&db_path, ServerRole::Query, SaltStoreMode::Db, true).await;
    assert_eq!(qry_state.ingest.capacity(), 0);
    let (qh_status, qh_body) = request(&qry_app, Method::GET, "/v1/health", None, &[], None).await;
    assert_eq!(qh_status, StatusCode::OK);
    let qh_json: Value = as_json(&qh_body);
    assert_eq!(qh_json["role"], "query");
    assert_eq!(qh_json["queue_capacity"], 0);
    assert_eq!(qh_json["queue_fill_pct"], 0.0);
    assert_eq!(
        request(&qry_app, Method::GET, "/v1/collect.js", None, &[], None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &qry_app,
            Method::POST,
            "/v1/events",
            None,
            &[],
            Some(&json!({}))
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &qry_app,
            Method::GET,
            "/v1/admin/sites",
            Some(ADMIN_KEY),
            &[],
            None
        )
        .await
        .0,
        StatusCode::OK
    );
}
