#![forbid(unsafe_code)]
//! Static Web UI serving (`--ui=on|off`, `--ui-dir <path>`, and `feature = "ui"` embedded assets).

use std::path::{Component, Path, PathBuf};

use axum::body::Body;
use axum::extract::State;
use axum::http::header::{ACCEPT_ENCODING, CACHE_CONTROL, CONTENT_ENCODING, CONTENT_TYPE, VARY};
use axum::http::{HeaderValue, Method, Request, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::auth_config::{UiConfig, UiMode};
use crate::config::ServerError;
use crate::error::ApiError;
use crate::state::AppState;

#[cfg(feature = "ui")]
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/embedded_ui.rs"));
}

pub(crate) fn validate_ui_config(ui: &UiConfig) -> Result<(), ServerError> {
    if ui.mode != UiMode::On {
        return Ok(());
    }
    if let Some(dir) = ui.dir.as_ref() {
        if !dir.is_dir() {
            return Err(ServerError::new(format!(
                "`--ui-dir` `{}` does not exist or is not a directory",
                dir.display()
            )));
        }
        return Ok(());
    }
    #[cfg(not(feature = "ui"))]
    {
        Err(ServerError::new(
            "`--ui=on` requires `--ui-dir <path>` or a server built with `--features ui`",
        ))
    }
    #[cfg(feature = "ui")]
    Ok(())
}

pub(crate) async fn serve_ui_fallback(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<Response, ApiError> {
    let method = req.method().clone();
    if method != Method::GET && method != Method::HEAD {
        return Ok(StatusCode::METHOD_NOT_ALLOWED.into_response());
    }
    let raw_path = req.uri().path();
    if raw_path == "/v1"
        || raw_path.starts_with("/v1/")
        || raw_path == "/mcp"
        || raw_path.starts_with("/mcp/")
    {
        return Err(ApiError::not_found("not_found", "unknown API endpoint"));
    }

    let rel = sanitize_rel_path(raw_path)?;
    let accept_gzip = req
        .headers()
        .get(ACCEPT_ENCODING)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| {
            v.split(',')
                .any(|s| s.trim().eq_ignore_ascii_case("gzip") || s.trim().starts_with("gzip;"))
        });

    let target = if rel.is_empty() { "index.html" } else { &rel };
    let loaded = load_asset(&state.ui, target, accept_gzip).await;
    let (served_rel, bytes, is_gzip) = match loaded {
        Some(hit) => hit,
        None if !target.starts_with("assets/") && !has_extension(target) => {
            let Some(spa) = load_asset(&state.ui, "index.html", accept_gzip).await else {
                return Ok(StatusCode::NOT_FOUND.into_response());
            };
            spa
        }
        None => return Ok(StatusCode::NOT_FOUND.into_response()),
    };

    let content_type = mime_for_path(&served_rel);
    let cache = if served_rel.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    let body = if method == Method::HEAD {
        Body::empty()
    } else {
        Body::from(bytes)
    };
    let mut resp = (StatusCode::OK, body).into_response();
    resp.headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    resp.headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static(cache));
    resp.headers_mut().insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    if is_gzip {
        resp.headers_mut()
            .insert(CONTENT_ENCODING, HeaderValue::from_static("gzip"));
        resp.headers_mut()
            .insert(VARY, HeaderValue::from_static("Accept-Encoding"));
    }
    Ok(resp)
}

fn sanitize_rel_path(raw_path: &str) -> Result<String, ApiError> {
    let trimmed = raw_path.trim_start_matches('/');
    if trimmed.contains('\\') || trimmed.contains('\0') {
        return Err(ApiError::bad_request("invalid_path", "invalid asset path"));
    }
    let mut parts = Vec::new();
    for comp in Path::new(trimmed).components() {
        match comp {
            Component::Normal(seg) => parts.push(seg.to_string_lossy().to_string()),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ApiError::bad_request(
                    "invalid_path",
                    "path traversal is forbidden",
                ));
            }
        }
    }
    Ok(parts.join("/"))
}

fn has_extension(rel: &str) -> bool {
    rel.rsplit('/').next().is_some_and(|seg| seg.contains('.'))
}

async fn load_asset(
    ui: &UiConfig,
    rel: &str,
    accept_gzip: bool,
) -> Option<(String, Vec<u8>, bool)> {
    if let Some(dir) = ui.dir.as_ref() {
        return load_from_dir(dir.clone(), rel.to_string(), accept_gzip).await;
    }
    #[cfg(feature = "ui")]
    {
        if accept_gzip {
            let gz_key = format!("{rel}.gz");
            if let Some((_, bytes)) = embedded::EMBEDDED_ASSETS.iter().find(|(p, _)| *p == gz_key) {
                return Some((rel.to_string(), (*bytes).to_vec(), true));
            }
        }
        embedded::EMBEDDED_ASSETS
            .iter()
            .find(|(p, _)| *p == rel)
            .map(|(_, bytes)| (rel.to_string(), (*bytes).to_vec(), false))
    }
    #[cfg(not(feature = "ui"))]
    {
        let _ = (rel, accept_gzip);
        None
    }
}

async fn load_from_dir(
    dir: PathBuf,
    rel: String,
    accept_gzip: bool,
) -> Option<(String, Vec<u8>, bool)> {
    tokio::task::spawn_blocking(move || {
        if accept_gzip {
            let gz_path = dir.join(format!("{rel}.gz"));
            if gz_path.is_file()
                && let Ok(bytes) = std::fs::read(&gz_path)
            {
                return Some((rel, bytes, true));
            }
        }
        let path = dir.join(&rel);
        if path.is_file()
            && let Ok(bytes) = std::fs::read(&path)
        {
            return Some((rel, bytes, false));
        }
        None
    })
    .await
    .ok()
    .flatten()
}

fn mime_for_path(rel: &str) -> &'static str {
    match rel.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "application/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "json" => "application/json; charset=utf-8",
        "webmanifest" => "application/manifest+json; charset=utf-8",
        "woff2" => "font/woff2",
        "wasm" => "application/wasm",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}
