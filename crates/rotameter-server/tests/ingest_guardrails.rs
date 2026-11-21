#![forbid(unsafe_code)]
//! Integration tests for ingest guardrails: origin allowlist, bot/prefetch drop, and `Sec-GPC`.

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::json;

use common::{as_json, in_memory_app, request};

#[tokio::test]
async fn guardrails_enforce_allowed_origins_bots_and_sec_gpc() {
    let admin = "rm_adm_guardrail_test_token_1234";
    let (state, app) = in_memory_app(admin);

    let (_, sb) = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "colinwolff.dev", "respect_gpc": true, "allowed_origins": ["https://colinwolff.dev"] })),
    )
    .await;
    let site_id = as_json(&sb)["site_id"].as_str().unwrap().to_string();

    let (st, _) = request(
        &app,
        Method::POST,
        "/v1/events",
        None,
        &[
            ("user-agent", "Mozilla/5.0 Chrome/131.0"),
            ("origin", "https://evil.example"),
        ],
        Some(&json!({ "site_id": site_id, "url": "https://colinwolff.dev/" })),
    )
    .await;
    assert_eq!(st, StatusCode::FORBIDDEN);

    let (st, bot_b) = request(
        &app,
        Method::POST,
        "/v1/events",
        None,
        &[("user-agent", "Googlebot/2.1")],
        Some(&json!({ "site_id": site_id, "url": "https://colinwolff.dev/" })),
    )
    .await;
    assert_eq!(
        (st, as_json(&bot_b)["accepted"].as_u64()),
        (StatusCode::ACCEPTED, Some(0))
    );

    let (st, gpc_b) = request(
        &app,
        Method::POST,
        "/v1/events",
        None,
        &[("user-agent", "Mozilla/5.0 Chrome/131.0"), ("sec-gpc", "1")],
        Some(&json!({ "site_id": site_id, "url": "https://colinwolff.dev/" })),
    )
    .await;
    assert_eq!(
        (st, as_json(&gpc_b)["accepted"].as_u64()),
        (StatusCode::ACCEPTED, Some(0))
    );

    state.ingest.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn trusted_proxy_headers_override_cf_only_when_authenticated() {
    let admin = "rm_adm_proxy_hdr_test_token_1234";
    let (state, app) = in_memory_app(admin);
    let (_, sb) = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "proxy.dev" })),
    )
    .await;
    let site_id = as_json(&sb)["site_id"].as_str().unwrap().to_string();
    let (_, kb) = request(
        &app,
        Method::POST,
        "/v1/admin/keys",
        Some(admin),
        &[],
        Some(&json!({ "scope": "ingest", "site": site_id, "label": "worker" })),
    )
    .await;
    let ing = as_json(&kb)["token"].as_str().unwrap().to_string();

    for ip in ["203.0.113.10", "203.0.113.11"] {
        let hdrs = [
            ("user-agent", "Mozilla/5.0 Chrome/131.0"),
            ("cf-connecting-ip", "198.51.100.1"),
            ("cf-ipcountry", "US"),
            ("x-rotameter-client-ip", ip),
            ("x-rotameter-country", "DE"),
            ("x-rotameter-region", "BE"),
        ];
        let (st, b) = request(
            &app,
            Method::POST,
            "/v1/events",
            Some(&ing),
            &hdrs,
            Some(&json!({ "site_id": site_id, "url": "https://proxy.dev/" })),
        )
        .await;
        assert_eq!(
            (st, as_json(&b)["accepted"].as_u64()),
            (StatusCode::ACCEPTED, Some(1))
        );
    }
    let unauth_hdrs = [
        ("user-agent", "Mozilla/5.0 Chrome/131.0"),
        ("cf-connecting-ip", "203.0.113.12"),
        ("cf-ipcountry", "CA"),
        ("x-rotameter-country", "FR"),
    ];
    assert_eq!(
        request(
            &app,
            Method::POST,
            "/v1/events",
            None,
            &unauth_hdrs,
            Some(&json!({ "site_id": site_id, "url": "https://proxy.dev/" }))
        )
        .await
        .0,
        StatusCode::ACCEPTED
    );
    state.ingest.flush_now().await;

    let (st, qb) = request(&app, Method::POST, "/v1/query", Some(admin), &[], Some(&json!({ "site_id": "proxy.dev", "range": { "preset": "24h" }, "metrics": ["visitors", "visits"], "group_by": ["country", "region"] }))).await;
    let q = as_json(&qb);
    assert_eq!(
        (
            st,
            q["summary"]["visitors"].as_u64(),
            q["rows"].as_array().unwrap().len()
        ),
        (StatusCode::OK, Some(3), 2)
    );
    assert_eq!(
        (
            q["rows"][0]["key"]["country"].as_str(),
            q["rows"][0]["key"]["region"].as_str(),
            q["rows"][0]["visitors"].as_u64()
        ),
        (Some("DE"), Some("BE"), Some(2))
    );
    assert_eq!(q["rows"][1]["key"]["country"].as_str(), Some("CA"));
    state.ingest.shutdown(Duration::from_secs(2)).await;
}
