#![forbid(unsafe_code)]
//! Integration tests for Cloudflare Access header forwarding and `/v1/events/batch`.

use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::post;
use axum::{Json, Router};
use rotameter_client::config::ClientConfig;
use rotameter_client::types::{BatchEventItem, BatchEventsRequest, SiteRef};
use rotameter_client::{ClientError, RotameterClient};
use tokio::net::TcpListener;

#[derive(Default)]
struct MockCapture {
    cf_id: Option<String>,
    cf_secret: Option<String>,
    auth: Option<String>,
    batch_len: usize,
}

#[tokio::test]
async fn cf_access_headers_and_batch_endpoint_and_limit_guard() {
    let captured = Arc::new(Mutex::new(MockCapture::default()));
    let app = Router::new()
        .route(
            "/v1/events/batch",
            post(
                |State(cap): State<Arc<Mutex<MockCapture>>>,
                 headers: HeaderMap,
                 Json(body): Json<BatchEventsRequest>| async move {
                    if let Ok(mut guard) = cap.lock() {
                        guard.cf_id = headers
                            .get("cf-access-client-id")
                            .and_then(|v| v.to_str().ok())
                            .map(ToString::to_string);
                        guard.cf_secret = headers
                            .get("cf-access-client-secret")
                            .and_then(|v| v.to_str().ok())
                            .map(ToString::to_string);
                        guard.auth = headers
                            .get("authorization")
                            .and_then(|v| v.to_str().ok())
                            .map(ToString::to_string);
                        guard.batch_len = body.events.len();
                    }
                    (
                        StatusCode::ACCEPTED,
                        Json(serde_json::json!({ "accepted": body.events.len(), "rejected": 0 })),
                    )
                },
            ),
        )
        .with_state(captured.clone());

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let srv = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let cfg = ClientConfig::new(&format!("http://{addr}"))
        .unwrap()
        .with_token("rm_ing_batch_token_123")
        .with_cf_access("cf-id-abc", "cf-secret-xyz");
    let client = RotameterClient::new(cfg).unwrap();

    let item = BatchEventItem {
        site_id: SiteRef::Slug("colinwolff.dev".to_string()),
        event: Some("pageview".to_string()),
        url: "https://colinwolff.dev/".to_string(),
        referrer: None,
        viewport_width: None,
        props: None,
        ts: Some("2026-11-22T12:00:00Z".to_string()),
        visitor_hint: Some("hint-1".to_string()),
        user_agent: None,
        country: None,
        ip: None,
    };
    let too_many = BatchEventsRequest {
        events: vec![item.clone(); 501],
    };
    assert!(matches!(
        client.post_events_batch(&too_many).await,
        Err(ClientError::InvalidRequest(_))
    ));

    let resp = client
        .post_events_batch(&BatchEventsRequest {
            events: vec![item.clone(), item],
        })
        .await
        .unwrap();
    assert_eq!((resp.accepted, resp.rejected), (2, 0));

    let guard = captured.lock().unwrap();
    assert_eq!(guard.cf_id.as_deref(), Some("cf-id-abc"));
    assert_eq!(guard.cf_secret.as_deref(), Some("cf-secret-xyz"));
    assert_eq!(guard.auth.as_deref(), Some("Bearer rm_ing_batch_token_123"));
    assert_eq!(guard.batch_len, 2);
    srv.abort();
}
