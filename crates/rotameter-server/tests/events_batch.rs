#![forbid(unsafe_code)]
//! Integration tests for authenticated `POST /v1/events/batch`.

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::json;

use common::{as_json, in_memory_app, request};

#[tokio::test]
async fn batch_ingest_auth_cap_mixed_items_and_visitor_hint() {
    let admin = "rm_adm_batch_srv_test_token_1234";
    let (state, app) = in_memory_app(admin).await;

    let (_, sb) = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "import.dev" })),
    )
    .await;
    let site_id = as_json(&sb)["site_id"].as_str().unwrap().to_string();
    let (_, kb) = request(
        &app,
        Method::POST,
        "/v1/admin/keys",
        Some(admin),
        &[],
        Some(&json!({ "scope": "read", "site": site_id, "label": "rd" })),
    )
    .await;
    let rd_tok = as_json(&kb)["token"].as_str().unwrap().to_string();

    assert_eq!(
        request(
            &app,
            Method::POST,
            "/v1/events/batch",
            None,
            &[],
            Some(&json!({ "events": [] }))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &app,
            Method::POST,
            "/v1/events/batch",
            Some(&rd_tok),
            &[],
            Some(&json!({ "events": [] }))
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    let big: Vec<_> = (0..501)
        .map(|_| json!({ "site_id": site_id, "url": "https://import.dev/" }))
        .collect();
    assert_eq!(
        request(
            &app,
            Method::POST,
            "/v1/events/batch",
            Some(admin),
            &[],
            Some(&json!({ "events": big }))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );

    let mixed = json!({
        "events": [
            { "site_id": "import.dev", "url": "https://import.dev/a", "visitor_hint": "h1" },
            { "site_id": "import.dev", "url": "https://import.dev/b", "visitor_hint": "h2" },
            { "site_id": "missing.dev", "url": "https://import.dev/c" },
            { "site_id": "import.dev", "url": "ftp://invalid.example" }
        ]
    });
    let (st, res_b) = request(
        &app,
        Method::POST,
        "/v1/events/batch",
        Some(admin),
        &[("cf-ipcountry", "GB")],
        Some(&mixed),
    )
    .await;
    assert_eq!(
        (
            st,
            as_json(&res_b)["accepted"].as_u64(),
            as_json(&res_b)["rejected"].as_u64()
        ),
        (StatusCode::ACCEPTED, Some(2), Some(2))
    );
    state.ingest.flush_now().await;

    let (_, qb) = request(&app, Method::POST, "/v1/query", Some(&rd_tok), &[], Some(&json!({ "site_id": "import.dev", "range": { "preset": "24h" }, "metrics": ["visitors", "pageviews"], "group_by": ["country"] }))).await;
    assert_eq!(
        (
            as_json(&qb)["summary"]["visitors"].as_u64(),
            as_json(&qb)["rows"][0]["key"]["country"].as_str()
        ),
        (Some(2), Some("GB"))
    );
    state.ingest.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn batch_ingest_enriches_per_item_ua_country_and_ephemeral_ip() {
    let admin = "rm_adm_batch_enrich_token_1234";
    let (state, app) = in_memory_app(admin).await;
    let _ = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "proxy.dev" })),
    )
    .await;

    let payload = json!({
        "events": [
            {
                "site_id": "proxy.dev",
                "url": "https://proxy.dev/docs",
                "ip": "203.0.113.10",
                "user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36",
                "country": "de"
            },
            {
                "site_id": "proxy.dev",
                "url": "https://proxy.dev/about",
                "ip": "203.0.113.10",
                "user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 Chrome/131.0.0.0 Safari/537.36",
                "country": "DE"
            },
            {
                "site_id": "proxy.dev",
                "url": "https://proxy.dev/bot-hit",
                "ip": "203.0.113.11",
                "user_agent": "Googlebot/2.1 (+http://www.google.com/bot.html)"
            }
        ]
    });
    let (st, res_b) = request(
        &app,
        Method::POST,
        "/v1/events/batch",
        Some(admin),
        &[],
        Some(&payload),
    )
    .await;
    assert_eq!(
        (
            st,
            as_json(&res_b)["accepted"].as_u64(),
            as_json(&res_b)["rejected"].as_u64()
        ),
        (StatusCode::ACCEPTED, Some(2), Some(1))
    );
    state.ingest.flush_now().await;

    let (_, qb) = request(
        &app,
        Method::POST,
        "/v1/query",
        Some(admin),
        &[],
        Some(&json!({
            "site_id": "proxy.dev",
            "range": { "preset": "24h" },
            "metrics": ["visitors", "pageviews"],
            "group_by": ["browser", "country"]
        })),
    )
    .await;
    let q = as_json(&qb);
    assert_eq!(q["summary"]["visitors"].as_u64(), Some(1));
    assert_eq!(q["summary"]["pageviews"].as_u64(), Some(2));
    assert_eq!(q["rows"][0]["key"]["country"].as_str(), Some("DE"));
    assert_eq!(q["rows"][0]["key"]["browser"].as_str(), Some("Chrome 131"));
    state.ingest.shutdown(Duration::from_secs(2)).await;
}
