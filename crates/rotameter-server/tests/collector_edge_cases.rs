#![forbid(unsafe_code)]
//! Expanded edge-case tests for `GET /v1/collect.js` and `GET /v1/pixel.gif`.

mod common;

use std::time::Duration;

use axum::body::Body;
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE};
use axum::http::{HeaderValue, Method, Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

use common::{as_json, in_memory_app, request};

#[tokio::test]
async fn collect_js_headers_body_and_privacy_invariants() {
    let admin = "rm_adm_collect_edge_token_1234";
    let (state, app) = in_memory_app(admin);

    let req = Request::builder()
        .method(Method::GET)
        .uri("/v1/collect.js")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get(CONTENT_TYPE),
        Some(&HeaderValue::from_static(
            "application/javascript; charset=utf-8"
        ))
    );
    assert_eq!(
        resp.headers().get(CACHE_CONTROL),
        Some(&HeaderValue::from_static("public, max-age=86400"))
    );
    assert_eq!(
        resp.headers().get("x-content-type-options"),
        Some(&HeaderValue::from_static("nosniff"))
    );

    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();
    let disk_js = include_str!("../assets/collect.js");
    assert_eq!(body, disk_js);
    assert!(!body.contains("document.cookie"));
    assert!(!body.contains("sessionStorage"));
    assert!(!body.contains("localStorage.setItem"));
    assert!(!body.contains("indexedDB"));

    state.ingest.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn pixel_gif_handles_missing_unknown_sites_and_custom_events() {
    let admin = "rm_adm_pixel_edge_token_123456";
    let (state, app) = in_memory_app(admin);

    for uri in ["/v1/pixel.gif", "/v1/pixel.gif?site=unknown-site.dev"] {
        let req = Request::builder()
            .method(Method::GET)
            .uri(uri)
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers().get(CONTENT_TYPE),
            Some(&HeaderValue::from_static("image/gif"))
        );
        assert_eq!(
            resp.headers().get(CACHE_CONTROL),
            Some(&HeaderValue::from_static("no-store, max-age=0"))
        );
        assert_eq!(
            resp.headers().get("x-content-type-options"),
            Some(&HeaderValue::from_static("nosniff"))
        );
        let bytes = resp.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(bytes.len(), 43);
        assert!(bytes.starts_with(b"GIF89a"));
    }

    let _ = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "edge.dev" })),
    )
    .await;

    let (st, bytes) = request(
        &app,
        Method::GET,
        "/v1/pixel.gif?site=edge.dev&path=feed/rss.xml&ref=https%3A%2F%2Fnews.ycombinator.com%2F&event=rss_read",
        None,
        &[("cf-connecting-ip", "203.0.113.45"), ("cf-ipcountry", "CA")],
        None,
    )
    .await;
    assert_eq!((st, bytes.len()), (StatusCode::OK, 43));

    state.ingest.flush_now().await;

    let (st_q, qb) = request(
        &app,
        Method::POST,
        "/v1/query",
        Some(admin),
        &[],
        Some(&json!({
            "site_id": "edge.dev",
            "range": { "preset": "24h" },
            "metrics": ["visitors", "visits"],
            "group_by": ["event_name", "path", "source"]
        })),
    )
    .await;
    assert_eq!(st_q, StatusCode::OK);
    let q = as_json(&qb);
    assert_eq!(q["summary"]["visitors"].as_u64(), Some(1));
    assert_eq!(q["rows"][0]["key"]["event_name"].as_str(), Some("rss_read"));
    assert_eq!(q["rows"][0]["key"]["path"].as_str(), Some("/feed/rss.xml"));
    assert_eq!(
        q["rows"][0]["key"]["source"].as_str(),
        Some("news.ycombinator.com")
    );

    state.ingest.shutdown(Duration::from_secs(2)).await;
}
