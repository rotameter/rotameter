#![forbid(unsafe_code)]
//! Integration tests for `GET /v1/collect.js` and zero-JS `GET /v1/pixel.gif`.

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::json;

use common::{as_json, in_memory_app, request};

#[tokio::test]
async fn collect_js_and_pixel_gif_endpoints_work() {
    let admin = "rm_adm_collect_test_token_123456";
    let (state, app) = in_memory_app(admin).await;

    let (st, js) = request(&app, Method::GET, "/v1/collect.js", None, &[], None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(String::from_utf8(js).unwrap().contains("rotameter_ignore"));

    let (_, sb) = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "rss" })),
    )
    .await;
    let site_id = as_json(&sb)["site_id"].as_str().unwrap().to_string();

    let (st, px) = request(
        &app,
        Method::GET,
        &format!("/v1/pixel.gif?site={site_id}&path=/feed.xml"),
        None,
        &[("cf-connecting-ip", "203.0.113.20"), ("cf-ipcountry", "DE")],
        None,
    )
    .await;
    assert_eq!((st, px.len()), (StatusCode::OK, 43));

    state.ingest.flush_now().await;
    let (st, rt) = request(
        &app,
        Method::GET,
        "/v1/sites/rss/realtime",
        Some(admin),
        &[],
        None,
    )
    .await;
    assert_eq!(
        (st, as_json(&rt)["active_visitors"].as_u64()),
        (StatusCode::OK, Some(1))
    );
    state.ingest.shutdown(Duration::from_secs(2)).await;
}
