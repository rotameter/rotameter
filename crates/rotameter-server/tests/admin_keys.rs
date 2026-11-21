#![forbid(unsafe_code)]
//! Integration tests for `/v1/admin/keys/*` minting, revocation, and scope enforcement.

mod common;

use std::time::Duration;

use axum::http::{Method, StatusCode};
use serde_json::json;

use common::{as_json, in_memory_app, request};

#[tokio::test]
async fn scoped_api_key_mint_and_authorization() {
    let admin = "rm_adm_key_scope_test_token_12345";
    let (state, app) = in_memory_app(admin);
    let _ = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "site-a" })),
    )
    .await;
    let _ = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "site-b" })),
    )
    .await;

    let (st, kb) = request(
        &app,
        Method::POST,
        "/v1/admin/keys",
        Some(admin),
        &[],
        Some(&json!({ "scope": "read", "site": "site-a", "label": "web" })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED);
    let rd_tok = as_json(&kb)["token"].as_str().unwrap().to_string();

    assert_eq!(
        request(
            &app,
            Method::GET,
            "/v1/admin/sites",
            Some(&rd_tok),
            &[],
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Method::GET,
            "/v1/sites/site-b/realtime",
            Some(&rd_tok),
            &[],
            None
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        request(
            &app,
            Method::GET,
            "/v1/sites/site-a/realtime",
            Some(&rd_tok),
            &[],
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    state.ingest.shutdown(Duration::from_secs(2)).await;
}

#[tokio::test]
async fn api_key_listing_and_revocation() {
    let admin = "rm_adm_key_revoke_test_token_1234";
    let (state, app) = in_memory_app(admin);
    let _ = request(
        &app,
        Method::POST,
        "/v1/admin/sites",
        Some(admin),
        &[],
        Some(&json!({ "slug": "site-a" })),
    )
    .await;
    let (_, kb) = request(
        &app,
        Method::POST,
        "/v1/admin/keys",
        Some(admin),
        &[],
        Some(&json!({ "scope": "read", "site": "site-a", "label": "web" })),
    )
    .await;
    let rd = as_json(&kb);
    let rd_tok = rd["token"].as_str().unwrap().to_string();
    let key_id = rd["key_id"].as_str().unwrap().to_string();

    let (_, list_b) = request(&app, Method::GET, "/v1/admin/keys", Some(admin), &[], None).await;
    assert_eq!(as_json(&list_b).as_array().unwrap().len(), 2);
    assert_eq!(
        request(
            &app,
            Method::DELETE,
            &format!("/v1/admin/keys/{key_id}"),
            Some(admin),
            &[],
            None
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        request(
            &app,
            Method::GET,
            "/v1/sites/site-a/realtime",
            Some(&rd_tok),
            &[],
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    state.ingest.shutdown(Duration::from_secs(2)).await;
}
