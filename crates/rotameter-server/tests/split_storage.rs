#![forbid(unsafe_code)]
//! Integration tests proving split OLTP [`ControlPlaneStore`] and OLAP [`AnalyticsDataPlane`]
//! isolation across two distinct `SQLite` databases via [`ServerStores::split`].

mod common;

use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use axum::http::{Method, StatusCode};
use rotameter_core::id::OrgId;
use rotameter_server::{AppState, ServerConfig, ServerStores, build_router};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::json;

use common::{TestStore, as_json, request};

#[tokio::test]
async fn split_control_plane_and_analytics_data_plane_stay_isolated() {
    let admin = "rm_adm_split_storage_test_token_1";
    let dir = tempfile::tempdir().unwrap();
    let control_sqlite = Arc::new(
        SqliteStore::open(
            dir.path().join("control.db"),
            SqliteConfig::default(),
            1_700_000_000_000,
        )
        .unwrap(),
    );
    let analytics_sqlite = Arc::new(
        SqliteStore::open(
            dir.path().join("analytics.db"),
            SqliteConfig::default(),
            1_700_000_000_000,
        )
        .unwrap(),
    );

    let stores = ServerStores::split(
        Arc::new(TestStore(Arc::clone(&control_sqlite))),
        Arc::new(TestStore(Arc::clone(&analytics_sqlite))),
    );
    let state = AppState::new(
        stores,
        &ServerConfig {
            bootstrap_admin_key: Some(admin.to_string()),
            self_site_enabled: false,
            ..ServerConfig::default()
        },
    )
    .await
    .unwrap();
    let app = build_router(state.clone());

    let (site_id, ing_tok) = seed_site_and_ingest_key(&app, admin).await.unwrap();
    let (st, ev_body) = request(
        &app,
        Method::POST,
        "/v1/events",
        Some(&ing_tok),
        &[("user-agent", "Mozilla/5.0 Chrome/131.0")],
        Some(&json!({ "site_id": site_id, "url": "https://split.dev/pricing" })),
    )
    .await;
    assert_eq!(st, StatusCode::ACCEPTED);
    assert_eq!(as_json(&ev_body)["accepted"].as_u64(), Some(1));
    state.ingest.flush_now().await;

    assert_query_and_realtime(&app, admin).await;
    assert_plane_isolation(&app, admin, &control_sqlite, &analytics_sqlite)
        .await
        .unwrap();
    state.ingest.shutdown(Duration::from_secs(2)).await;
}

async fn seed_site_and_ingest_key(
    app: &Router,
    admin: &str,
) -> Result<(String, String), Box<dyn Error>> {
    let (st, site_body) = request(
        app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "split.dev" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let site_id = as_json(&site_body)["site_id"]
        .as_str()
        .ok_or("missing site_id")?
        .to_string();

    let (st, key_body) = request(
        app,
        Method::POST,
        "/v1/admin/keys",
        Some(admin),
        &[],
        Some(&json!({ "scope": "ingest", "site": "split.dev", "label": "edge" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let ing_tok = as_json(&key_body)["token"]
        .as_str()
        .ok_or("missing token")?
        .to_string();
    Ok((site_id, ing_tok))
}

async fn assert_query_and_realtime(app: &Router, admin: &str) {
    let (st, q_body) = request(
        app,
        Method::POST,
        "/v1/query",
        Some(admin),
        &[],
        Some(&json!({
            "site_id": "split.dev",
            "range": { "preset": "24h" },
            "metrics": ["visitors", "pageviews", "visits"]
        })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&q_body)["summary"]["pageviews"].as_u64(), Some(1));

    let (st, rt_body) = request(
        app,
        Method::GET,
        "/v1/sites/split.dev/realtime",
        Some(admin),
        &[],
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&rt_body)["active_visitors"].as_u64(), Some(1));
}

async fn assert_plane_isolation(
    app: &Router,
    admin: &str,
    control_sqlite: &SqliteStore,
    analytics_sqlite: &SqliteStore,
) -> Result<(), Box<dyn Error>> {
    let ctrl_org = control_sqlite.ensure_default_org_sync(1_700_000_000_000)?;
    let sites_in_ctrl = control_sqlite.list_sites_sync(ctrl_org.org_id)?;
    assert_eq!(sites_in_ctrl.len(), 1);
    let site_obj = sites_in_ctrl[0].clone();
    assert_eq!(
        control_sqlite
            .realtime_sync(&site_obj, 1_700_000_000_000)?
            .active_visitors,
        0
    );

    let default_org_bytes = OrgId::from_bytes(*ctrl_org.org_id.as_bytes());
    assert_eq!(
        analytics_sqlite.list_sites_sync(default_org_bytes)?.len(),
        0
    );
    assert_eq!(
        analytics_sqlite
            .list_api_keys_sync(default_org_bytes)?
            .len(),
        0
    );

    let (st, del_body) = request(
        app,
        Method::DELETE,
        "/v1/admin/sites/split.dev?confirm=true",
        Some(admin),
        &[],
        None,
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(as_json(&del_body)["deleted"].as_bool(), Some(true));
    assert_eq!(
        analytics_sqlite
            .realtime_sync(&site_obj, 1_700_000_000_000)?
            .active_visitors,
        0
    );
    Ok(())
}
