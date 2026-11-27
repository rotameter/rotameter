#![forbid(unsafe_code)]
//! Integration tests for Cloudflare Access JWT subject validation.

mod common;

use std::sync::Arc;

use axum::http::{Method, StatusCode};
use rotameter_server::{AppState, OperatorAuthConfig, ServerConfig, ServerStores, build_router};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use serde_json::json;

use crate::common::{TestStore, as_json, request};

fn fake_jwt(payload: &serde_json::Value) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let b64 = |data: &[u8]| -> String {
        let mut out = String::new();
        for chunk in data.chunks(3) {
            let b0 = u32::from(chunk[0]);
            let b1 = u32::from(*chunk.get(1).unwrap_or(&0));
            let b2 = u32::from(*chunk.get(2).unwrap_or(&0));
            let t = (b0 << 16) | (b1 << 8) | b2;
            out.push(char::from(TABLE[((t >> 18) & 0x3f) as usize]));
            out.push(char::from(TABLE[((t >> 12) & 0x3f) as usize]));
            if chunk.len() > 1 {
                out.push(char::from(TABLE[((t >> 6) & 0x3f) as usize]));
            }
            if chunk.len() > 2 {
                out.push(char::from(TABLE[(t & 0x3f) as usize]));
            }
        }
        out
    };
    let hdr = b64(br#"{"alg":"RS256"}"#);
    let body = b64(&serde_json::to_vec(payload).unwrap_or_default());
    format!("{hdr}.{body}.sig")
}

#[tokio::test]
async fn cf_access_jwt_rejects_empty_service_token_subject_and_accepts_user_email() {
    let Ok(sqlite) = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000) else {
        std::process::abort()
    };
    let store = TestStore::new(sqlite);
    let cfg = ServerConfig {
        operator_auth: OperatorAuthConfig {
            cf_access_trusted: true,
            cf_access_team_domain: Some("https://wolffindustries.cloudflareaccess.com".into()),
            cf_access_aud: Some("aud-123".into()),
            ..OperatorAuthConfig::default()
        },
        ..ServerConfig::default()
    };
    let Ok(state) = AppState::new(ServerStores::unified(Arc::clone(&store)), &cfg).await else {
        std::process::abort()
    };
    let app = build_router(state);

    let service_jwt = fake_jwt(&json!({
        "aud": ["aud-123"],
        "iss": "https://wolffindustries.cloudflareaccess.com",
        "sub": "",
        "common_name": "service-token-client-id.access"
    }));
    let (st_me, _) = request(
        &app,
        Method::GET,
        "/v1/auth/me",
        None,
        &[("cf-access-jwt-assertion", &service_jwt)],
        None,
    )
    .await;
    assert_eq!(st_me, StatusCode::UNAUTHORIZED);

    let (st_admin, _) = request(
        &app,
        Method::GET,
        "/v1/admin/sites",
        None,
        &[("cf-access-jwt-assertion", &service_jwt)],
        None,
    )
    .await;
    assert_eq!(st_admin, StatusCode::UNAUTHORIZED);

    let user_jwt = fake_jwt(&json!({
        "aud": ["aud-123"],
        "iss": "https://wolffindustries.cloudflareaccess.com",
        "email": "lumewolff@gmail.com",
        "sub": "user-uuid"
    }));
    let (st_ok, body_ok) = request(
        &app,
        Method::GET,
        "/v1/auth/me",
        None,
        &[("cf-access-jwt-assertion", &user_jwt)],
        None,
    )
    .await;
    assert_eq!(st_ok, StatusCode::OK);
    assert_eq!(as_json(&body_ok)["method"], "cf_access");
    assert_eq!(as_json(&body_ok)["subject"], "lumewolff@gmail.com");
}
