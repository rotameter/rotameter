#![forbid(unsafe_code)]
//! Integration tests for per-IP rate limiting and `GeoIP` modes (`MMDB` & proxy header trust).

mod common;

use std::time::Duration;

use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use rotameter_server::{
    AppState, GeoIpMode, RateLimitConfig, ServerConfig, ServerStores, build_router,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::json;
use tower::ServiceExt;

use common::{TestStore, as_json, request};

#[tokio::test]
async fn rate_limiter_enforces_429_and_bypasses_authenticated_ingest() {
    let admin = "rm_adm_ratelimit_test_token_1234";
    let sqlite = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000).unwrap();
    let state = AppState::new(
        ServerStores::unified(TestStore::new(sqlite)),
        &ServerConfig {
            bootstrap_admin_key: Some(admin.to_string()),
            self_site_enabled: false,
            rate_limit: RateLimitConfig {
                enabled: true,
                requests_per_minute: 60,
                burst: 2,
                max_tracked_ips: 64,
            },
            ..ServerConfig::default()
        },
    )
    .await
    .unwrap();
    let app = build_router(state.clone());

    for _ in 0..2 {
        let (st, _) = request(
            &app,
            Method::GET,
            "/v1/collect.js",
            None,
            &[("cf-connecting-ip", "203.0.113.5")],
            None,
        )
        .await;
        assert_eq!(st, StatusCode::OK);
    }
    let req = Request::builder()
        .method(Method::GET)
        .uri("/v1/collect.js")
        .header("cf-connecting-ip", "203.0.113.5")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(resp.headers().contains_key(header::RETRY_AFTER));
    let body = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        as_json(&body)["error"]["code"].as_str(),
        Some("rate_limited")
    );

    let (st_other, _) = request(
        &app,
        Method::GET,
        "/v1/collect.js",
        None,
        &[("cf-connecting-ip", "203.0.113.99")],
        None,
    )
    .await;
    assert_eq!(st_other, StatusCode::OK);

    let (_, sb) = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "rl.dev" })),
    )
    .await;
    let site_id = as_json(&sb)["site_id"].as_str().unwrap().to_string();
    let (st_auth, _) = request(
        &app,
        Method::POST,
        "/v1/events",
        Some(admin),
        &[
            ("user-agent", "Mozilla/5.0 Chrome/131.0"),
            ("cf-connecting-ip", "203.0.113.5"),
        ],
        Some(&json!({ "site_id": site_id, "url": "https://rl.dev/" })),
    )
    .await;
    assert_eq!(st_auth, StatusCode::ACCEPTED);
    state.ingest.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn mmdb_geoip_requires_valid_path_and_untrusted_proxy_ignores_cf_headers() {
    let sqlite = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000).unwrap();
    let store = TestStore::new(sqlite);
    assert!(
        AppState::new(
            ServerStores::unified(store.clone()),
            &ServerConfig {
                geoip_mode: GeoIpMode::Mmdb,
                self_site_enabled: false,
                ..ServerConfig::default()
            }
        )
        .await
        .is_err()
    );
    assert!(
        AppState::new(
            ServerStores::unified(store.clone()),
            &ServerConfig {
                geoip_mode: GeoIpMode::Mmdb,
                mmdb_path: Some("/nonexistent/GeoLite2.mmdb".into()),
                self_site_enabled: false,
                ..ServerConfig::default()
            }
        )
        .await
        .is_err()
    );

    let admin = "rm_adm_untrusted_proxy_test_1234";
    let state = AppState::new(
        ServerStores::unified(store),
        &ServerConfig {
            bootstrap_admin_key: Some(admin.to_string()),
            self_site_enabled: false,
            trusted_proxy_headers: false,
            ..ServerConfig::default()
        },
    )
    .await
    .unwrap();
    let app = build_router(state.clone());
    request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "untrusted.dev" })),
    )
    .await;
    request(
        &app,
        Method::POST,
        "/v1/events",
        None,
        &[
            ("user-agent", "Mozilla/5.0 Chrome/131.0"),
            ("cf-ipcountry", "US"),
        ],
        Some(&json!({ "site_id": "untrusted.dev", "url": "https://untrusted.dev/" })),
    )
    .await;
    state.ingest.flush_now().await;
    let (_, qb) = request(&app, Method::POST, "/v1/query", Some(admin), &[], Some(&json!({ "site_id": "untrusted.dev", "range": { "preset": "24h" }, "metrics": ["visitors"], "group_by": ["country"] }))).await;
    assert_eq!(
        as_json(&qb)["rows"][0]["key"]["country"].as_str(),
        Some("(unknown)")
    );
    state.ingest.shutdown(Duration::from_secs(2)).await;
}
