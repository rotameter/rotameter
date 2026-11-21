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
