#![forbid(unsafe_code)]
//! Non-blocking `POST /v1/events` ingestion, normalization, bot/`Sec-GPC`/origin checks.

use std::collections::BTreeMap;

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use rotameter_core::bot::{BotFilterDecision, PrefetchHeaders, classify_traffic};
use rotameter_core::id::SiteRef;
use rotameter_core::normalize::{normalize_referrer, normalize_url};
use rotameter_core::privacy::resolve_visitor_candidates;
use rotameter_core::types::{EventKind, KeyScope, ViewportClass};
use rotameter_core::ua::classify_user_agent;
use serde::Deserialize;

use crate::auth::{AuthContext, extract_optional_auth};
use crate::error::ApiError;
use crate::handlers::headers::{
    extract_client_ip, extract_country, extract_region, header_str, parse_props,
};
use crate::ingest::types::QueuedEvent;
use crate::state::{AppState, now_ms};

#[derive(Deserialize)]
pub(crate) struct EventRequest {
    site_id: String,
    event: Option<String>,
    url: String,
    referrer: Option<String>,
    viewport_width: Option<u16>,
    #[serde(default)]
    props: Option<BTreeMap<String, serde_json::Value>>,
}

pub(crate) struct HitInput<'a> {
    pub(crate) site_id_raw: &'a str,
    pub(crate) event_name: Option<&'a str>,
    pub(crate) url: &'a str,
    pub(crate) referrer: Option<&'a str>,
    pub(crate) viewport_width: Option<u16>,
    pub(crate) props: Option<&'a BTreeMap<String, serde_json::Value>>,
    pub(crate) require_user_agent: bool,
}

pub(crate) async fn post_event(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let req: EventRequest = serde_json::from_slice(&body).map_err(|err| {
        ApiError::bad_request("invalid_json", format!("malformed payload: {err}"))
    })?;
    let auth = extract_optional_auth(&state, &headers)?;
    let accepted = enqueue_normalized_hit(
        &state,
        &headers,
        auth,
        &HitInput {
            site_id_raw: &req.site_id,
            event_name: req.event.as_deref(),
            url: &req.url,
            referrer: req.referrer.as_deref(),
            viewport_width: req.viewport_width,
            props: req.props.as_ref(),
            require_user_agent: auth.is_none(),
        },
    )?;
    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "accepted": u8::from(accepted) })),
    ))
}

pub(crate) fn enqueue_normalized_hit(
    state: &AppState,
    headers: &HeaderMap,
    auth: Option<AuthContext>,
    hit: &HitInput<'_>,
) -> Result<bool, ApiError> {
    let site_ref = SiteRef::parse(hit.site_id_raw).map_err(ApiError::from)?;
    let org_id = match auth {
        Some(ctx) => {
            if !matches!(ctx.scope, KeyScope::Ingest | KeyScope::Admin) {
                return Err(ApiError::forbidden(
                    "insufficient_scope",
                    "key cannot ingest events",
                ));
            }
            ctx.org_id
        }
        None => state.default_org_id,
    };

    let site = state
        .store
        .resolve_site(org_id, &site_ref)
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("site_not_found", "unknown site identifier"))?;

    if let Some(ctx) = auth {
        if ctx.scope == KeyScope::Ingest && ctx.site_id != Some(site.site_id) {
            return Err(ApiError::forbidden(
                "site_scope_mismatch",
                "ingest key is not scoped to the target site",
            ));
        }
    } else if !site.allowed_origins.is_empty()
        && let Some(origin) =
            header_str(headers, "origin").or_else(|| header_str(headers, "referer"))
    {
        let origin_url = if origin.contains("://") {
            origin
        } else {
            format!("https://{origin}")
        };
        normalize_url(&origin_url, &[], &site.allowed_origins).map_err(ApiError::from)?;
    }

    if site.respect_gpc && header_str(headers, "sec-gpc").as_deref() == Some("1") {
        return Ok(false);
    }

    let ua = header_str(headers, "user-agent");
    let prefetch = PrefetchHeaders {
        sec_purpose: headers.get("sec-purpose").and_then(|v| v.to_str().ok()),
        purpose: headers.get("purpose").and_then(|v| v.to_str().ok()),
        x_moz: headers.get("x-moz").and_then(|v| v.to_str().ok()),
    };
    if matches!(
        classify_traffic(ua.as_deref(), prefetch, hit.require_user_agent, false),
        BotFilterDecision::Drop(_)
    ) {
        return Ok(false);
    }

    let event_kind =
        EventKind::parse(hit.event_name.unwrap_or("pageview")).map_err(ApiError::from)?;
    let norm_url = normalize_url(hit.url, &site.allowed_query_keys, &site.allowed_origins)
        .map_err(ApiError::from)?;
    let norm_ref = normalize_referrer(
        hit.referrer,
        &norm_url.host,
        &site.allowed_origins,
        &norm_url.utm,
    );
    let props = parse_props(hit.props)?;

    let ts_ms = now_ms();
    let ip = extract_client_ip(headers);
    let ua_str = ua.as_deref().unwrap_or_default();
    let candidates = resolve_visitor_candidates(
        state.salt_provider.as_ref(),
        site.site_id,
        site.salt_rotation,
        ts_ms,
        &ip,
        ua_str,
    )
    .map_err(ApiError::from)?;
    let classified_ua = classify_user_agent(ua_str);

    state.ingest.try_enqueue(QueuedEvent {
        site_id: site.site_id,
        ts_ms,
        candidates,
        event_kind,
        path: norm_url.path,
        props,
        ref_host: norm_ref.source,
        channel: norm_ref.channel,
        utm: norm_url.utm,
        country: extract_country(headers),
        region: extract_region(headers),
        browser: classified_ua.browser,
        os: classified_ua.os,
        device_type: classified_ua.device_type,
        viewport_class: ViewportClass::from_width(hit.viewport_width),
    })?;
    Ok(true)
}
