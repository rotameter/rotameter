#![forbid(unsafe_code)]
//! First-party `GET /v1/collect.js` tracker asset and `GET /v1/pixel.gif` fallback handler.

use axum::extract::{Query, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, X_CONTENT_TYPE_OPTIONS};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use rotameter_core::id::SiteRef;
use serde::Deserialize;

use crate::auth::extract_optional_auth;
use crate::error::ApiError;
use crate::handlers::events::{HitInput, enqueue_normalized_hit};
use crate::handlers::headers::extract_client_ip;
use crate::state::AppState;

const COLLECT_JS: &str = include_str!("../../../../assets/collect.js");

const PIXEL_GIF: [u8; 43] = [
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x01, 0x00, 0x01, 0x00, 0x80, 0x00, 0x00, 0xff, 0xff, 0xff,
    0x00, 0x00, 0x00, 0x21, 0xf9, 0x04, 0x01, 0x00, 0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x01, 0x00, 0x00, 0x02, 0x02, 0x44, 0x01, 0x00, 0x3b,
];

#[derive(Deserialize)]
pub(crate) struct PixelQuery {
    site: Option<String>,
    path: Option<String>,
    #[serde(rename = "ref")]
    referrer: Option<String>,
    event: Option<String>,
}

pub(crate) async fn get_collect_js(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    if extract_optional_auth(&state, &headers)
        .ok()
        .flatten()
        .is_none()
    {
        let ip = extract_client_ip(&headers, false);
        state.rate_limiter.check(&ip).map_err(|wait| {
            ApiError::too_many_requests("rate_limited", "rate limit exceeded", wait)
        })?;
    }
    Ok((
        StatusCode::OK,
        [
            (CONTENT_TYPE, "application/javascript; charset=utf-8"),
            (CACHE_CONTROL, "public, max-age=86400"),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        COLLECT_JS,
    ))
}

pub(crate) async fn get_pixel_gif(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PixelQuery>,
) -> Result<impl IntoResponse, ApiError> {
    let auth = extract_optional_auth(&state, &headers).ok().flatten();
    if auth.is_none() {
        let ip = extract_client_ip(&headers, false);
        state.rate_limiter.check(&ip).map_err(|wait| {
            ApiError::too_many_requests("rate_limited", "rate limit exceeded", wait)
        })?;
    }
    if let Some(site_raw) = query.site.as_deref() {
        let org_id = auth.map_or(state.default_org_id, |ctx| ctx.org_id);
        let host = SiteRef::parse(site_raw)
            .ok()
            .and_then(|s_ref| state.store.resolve_site(org_id, &s_ref).ok().flatten())
            .and_then(|site| site.allowed_origins.into_iter().next())
            .map_or_else(
                || "pixel.rotameter.local".to_string(),
                |origin| {
                    origin
                        .trim_start_matches("https://")
                        .trim_start_matches("http://")
                        .split('/')
                        .next()
                        .unwrap_or("pixel.rotameter.local")
                        .to_string()
                },
            );
        let raw_path = query.path.as_deref().unwrap_or("/");
        let slash_path = if raw_path.starts_with('/') {
            raw_path.to_string()
        } else {
            format!("/{raw_path}")
        };
        let url = format!("https://{host}{slash_path}");
        let _ = enqueue_normalized_hit(
            &state,
            &headers,
            auth,
            &HitInput {
                site_id_raw: site_raw,
                event_name: query.event.as_deref(),
                url: &url,
                referrer: query.referrer.as_deref(),
                viewport_width: None,
                props: None,
                require_user_agent: false,
            },
        );
    }

    Ok((
        StatusCode::OK,
        [
            (CONTENT_TYPE, "image/gif"),
            (CACHE_CONTROL, "no-store, max-age=0"),
            (X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        PIXEL_GIF,
    ))
}
