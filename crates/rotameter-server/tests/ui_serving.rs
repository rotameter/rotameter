#![forbid(unsafe_code)]
//! Integration tests for `--ui=on|off`, `--ui-dir` static SPA serving, and `feature = "ui"`.

mod common;

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
use rotameter_server::{AppState, ServerConfig, ServerStores, UiConfig, UiMode, build_router};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use tower::ServiceExt;

use crate::common::{TestStore, as_json};

async fn ui_app(ui: UiConfig) -> axum::Router {
    let Ok(sqlite) = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000) else {
        std::process::abort()
    };
    let store = TestStore::new(sqlite);
    let Ok(state) = AppState::new(
        ServerStores::unified(Arc::clone(&store)),
        &ServerConfig {
            ui,
            ..ServerConfig::default()
        },
    )
    .await
    else {
        std::process::abort()
    };
    build_router(state)
}

async fn get_with_headers(
    app: &axum::Router,
    uri: &str,
    headers: &[(&str, &str)],
) -> (StatusCode, axum::http::HeaderMap, Vec<u8>) {
    let mut b = Request::builder().method(Method::GET).uri(uri);
    for (k, v) in headers {
        b = b.header(*k, *v);
    }
    let Ok(req) = b.body(Body::empty()) else {
        std::process::abort()
    };
    let Ok(resp) = app.clone().oneshot(req).await;
    let status = resp.status();
    let hdrs = resp.headers().clone();
    let Ok(bytes) = resp.into_body().collect().await else {
        std::process::abort()
    };
    (status, hdrs, bytes.to_bytes().to_vec())
}

#[tokio::test]
async fn ui_disabled_returns_404_and_ui_dir_serves_spa_and_assets() {
    let off_app = ui_app(UiConfig::default()).await;
    let (off_st, _, _) = get_with_headers(&off_app, "/", &[]).await;
    assert_eq!(off_st, StatusCode::NOT_FOUND);

    let Ok(dir) = tempfile::tempdir() else {
        std::process::abort()
    };
    let assets = dir.path().join("assets");
    assert!(std::fs::create_dir_all(&assets).is_ok());
    assert!(std::fs::write(dir.path().join("index.html"), b"<html>app</html>").is_ok());
    assert!(std::fs::write(assets.join("app.js"), b"console.log('raw');").is_ok());
    assert!(std::fs::write(assets.join("app.js.gz"), b"gzip-bytes").is_ok());

    let app = ui_app(UiConfig {
        mode: UiMode::On,
        dir: Some(dir.path().to_path_buf()),
    })
    .await;

    let (idx_st, idx_hdrs, idx_body) = get_with_headers(&app, "/", &[]).await;
    assert_eq!(idx_st, StatusCode::OK);
    assert_eq!(idx_body, b"<html>app</html>");
    assert_eq!(
        idx_hdrs
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-cache")
    );
    assert!(idx_hdrs.get(header::SET_COOKIE).is_none());

    let (spa_st, _, spa_body) = get_with_headers(&app, "/sites/example.com/overview", &[]).await;
    assert_eq!(spa_st, StatusCode::OK);
    assert_eq!(spa_body, b"<html>app</html>");

    let (js_st, js_hdrs, js_body) =
        get_with_headers(&app, "/assets/app.js", &[("accept-encoding", "gzip")]).await;
    assert_eq!(js_st, StatusCode::OK);
    assert_eq!(js_body, b"gzip-bytes");
    assert_eq!(
        js_hdrs
            .get(header::CONTENT_ENCODING)
            .and_then(|v| v.to_str().ok()),
        Some("gzip")
    );
    assert_eq!(
        js_hdrs
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("public, max-age=31536000, immutable")
    );

    let (miss_asset_st, _, _) = get_with_headers(&app, "/assets/missing.js", &[]).await;
    assert_eq!(miss_asset_st, StatusCode::NOT_FOUND);

    let (trav_st, _, _) = get_with_headers(&app, "/../secret", &[]).await;
    assert_eq!(trav_st, StatusCode::BAD_REQUEST);

    let (v1_st, _, v1_body) = get_with_headers(&app, "/v1/does-not-exist", &[]).await;
    assert_eq!(v1_st, StatusCode::NOT_FOUND);
    assert_eq!(as_json(&v1_body)["error"]["code"], "not_found");
}

#[cfg(feature = "ui")]
#[tokio::test]
async fn embedded_ui_feature_serves_index_html() {
    let app = ui_app(UiConfig {
        mode: UiMode::On,
        dir: None,
    })
    .await;
    let (st, hdrs, body) = get_with_headers(&app, "/", &[]).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(
        hdrs.get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("no-cache")
    );
    assert_ne!(body, [] as [u8; 0]);
}
