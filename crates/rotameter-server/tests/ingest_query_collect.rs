#![forbid(unsafe_code)]
//! Integration tests for `POST /v1/events`, `GeoIP` headers, `POST /v1/query`, and realtime.

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::json;

use common::{as_json, in_memory_app, request};

#[tokio::test]
async fn end_to_end_ingest_flush_query_csv_and_realtime() {
    let admin = "rm_adm_e2e_bootstrap_token_987654";
    let (state, app) = in_memory_app(admin);

    let (_, sb) = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "colinwolff.dev" })),
    )
    .await;
    let site_id = as_json(&sb)["site_id"].as_str().unwrap().to_string();
    let (_, kb) = request(
        &app,
        Method::POST,
        "/v1/admin/keys",
        Some(admin),
        &[],
        Some(&json!({ "scope": "read", "site": site_id, "label": "dash" })),
    )
    .await;
    let rd_tok = as_json(&kb)["token"].as_str().unwrap().to_string();

    let (st, ev_b) = request(
        &app,
        Method::POST,
        "/v1/events",
        None,
        &[
            ("user-agent", "Mozilla/5.0 Chrome/131.0"),
            ("cf-connecting-ip", "203.0.113.9"),
            ("cf-ipcountry", "US"),
            ("cf-region-code", "OR"),
        ],
        Some(&json!({ "site_id": site_id, "url": "https://colinwolff.dev/projects/rotameter" })),
    )
    .await;
    assert_eq!(
        (st, as_json(&ev_b)["accepted"].as_u64()),
        (StatusCode::ACCEPTED, Some(1))
    );
    state.ingest.flush_now().await;

    let (st, qb) = request(
        &app,
        Method::POST,
        "/v1/query",
        Some(&rd_tok),
        &[],
        Some(&json!({ "site_id": "colinwolff.dev", "range": { "preset": "24h" }, "granularity": "hour", "metrics": ["visitors", "pageviews"], "group_by": ["country"] })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let q = as_json(&qb);
    assert_eq!(
        (
            q["summary"]["visitors"].as_u64(),
            q["rows"][0]["key"]["country"].as_str()
        ),
        (Some(1), Some("US"))
    );

    let (st, csv) = request(
        &app,
        Method::POST,
        "/v1/query",
        Some(&rd_tok),
        &[],
        Some(&json!({ "site_id": "colinwolff.dev", "range": { "preset": "24h" }, "metrics": ["visits"], "group_by": ["path"], "format": "csv" })),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    assert!(
        String::from_utf8(csv)
            .unwrap()
            .contains("/projects/rotameter,1\n")
    );

    let (st, rt) = request(
        &app,
        Method::GET,
        "/v1/sites/colinwolff.dev/realtime",
        Some(&rd_tok),
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
