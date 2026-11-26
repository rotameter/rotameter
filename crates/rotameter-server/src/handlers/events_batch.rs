#![forbid(unsafe_code)]
//! Authenticated server-to-server `POST /v1/events/batch` ingestion (`<= 500` events).

use std::borrow::Cow;

use axum::Json;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use chrono::DateTime;
use rotameter_core::bot::{BotFilterDecision, PrefetchHeaders, classify_traffic};
use rotameter_core::id::SiteRef;
use rotameter_core::normalize::{
    NormalizedReferrer, NormalizedUrl, normalize_referrer, normalize_url,
};
use rotameter_core::privacy::resolve_visitor_candidates;
use rotameter_core::types::{EventKind, EventProps, KeyScope, Site, ViewportClass};
use rotameter_core::ua::{ClassifiedUa, classify_user_agent};

use crate::auth::{AuthContext, extract_optional_auth};
use crate::error::ApiError;
use crate::handlers::batch_wire::{BatchEventItem, BatchEventsRequest, MAX_BATCH_EVENTS};
use crate::handlers::headers::{
    extract_client_ip, extract_prefetch, header_str, normalize_country, parse_props,
};
use crate::ingest::types::QueuedEvent;
use crate::state::{AppState, now_ms};

struct BatchContext<'a> {
    auth: AuthContext,
    req_ip: &'a str,
    ua_str: &'a str,
    prefetch: PrefetchHeaders<'a>,
    gpc_header: bool,
    country: &'a str,
    region: &'a str,
    classified_ua: &'a ClassifiedUa,
}

pub(crate) async fn post_events_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<BatchEventsRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let auth = extract_optional_auth(&state, &headers)
        .await?
        .ok_or_else(|| ApiError::unauthorized("missing Bearer API key"))?;
    if !matches!(auth.scope, KeyScope::Ingest | KeyScope::Admin) {
        return Err(ApiError::forbidden(
            "insufficient_scope",
            "key cannot ingest batch events",
        ));
    }
    if req.events.len() > MAX_BATCH_EVENTS {
        return Err(ApiError::bad_request(
            "batch_too_large",
            format!("batch exceeds {MAX_BATCH_EVENTS}-event limit"),
        ));
    }

    let req_ip = extract_client_ip(&headers, true);
    let ua = header_str(&headers, "user-agent");
    let ua_str = ua.as_deref().unwrap_or_default();
    let (country, region) = state.geoip.resolve(&headers, &req_ip, true);
    let classified_ua = classify_user_agent(ua_str);
    let ctx = BatchContext {
        auth,
        req_ip: &req_ip,
        ua_str,
        prefetch: extract_prefetch(&headers),
        gpc_header: header_str(&headers, "sec-gpc").as_deref() == Some("1"),
        country: &country,
        region: &region,
        classified_ua: &classified_ua,
    };

    let (mut accepted, mut rejected) = (0_u64, 0_u64);
    for item in req.events {
        match build_batch_queued(&state, &ctx, &item).await? {
            Some(queued) => {
                state.ingest.try_enqueue(queued)?;
                accepted += 1;
            }
            None => rejected += 1,
        }
    }

    Ok((
        StatusCode::ACCEPTED,
        Json(serde_json::json!({ "accepted": accepted, "rejected": rejected })),
    ))
}

async fn build_batch_queued(
    state: &AppState,
    ctx: &BatchContext<'_>,
    item: &BatchEventItem,
) -> Result<Option<QueuedEvent>, ApiError> {
    let Ok(site_ref) = SiteRef::parse(&item.site_id) else {
        return Ok(None);
    };
    let Some(site) = state
        .control_plane
        .resolve_site(ctx.auth.org_id, &site_ref)
        .await
        .map_err(ApiError::from)?
    else {
        return Ok(None);
    };
    if ctx.auth.scope == KeyScope::Ingest && ctx.auth.site_id != Some(site.site_id) {
        return Ok(None);
    }
    let item_ua = item
        .user_agent
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let effective_ua = item_ua.unwrap_or(ctx.ua_str);
    let Some((ts_ms, event_kind, norm_url, norm_ref, props)) =
        normalize_batch_item(&site, ctx, item, effective_ua)
    else {
        return Ok(None);
    };
    let item_ip = item.ip.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let (identity, ua_hash) = if let Some(ip) = item_ip {
        (ip, effective_ua)
    } else if let Some(hint) = item
        .visitor_hint
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        (hint, "")
    } else {
        (ctx.req_ip, effective_ua)
    };
    let candidates = resolve_visitor_candidates(
        state.salt_provider.as_ref(),
        site.site_id,
        site.salt_rotation,
        ts_ms,
        identity,
        ua_hash,
    )
    .map_err(ApiError::from)?;
    let classified_ua = match item_ua {
        Some(ua) => Cow::Owned(classify_user_agent(ua)),
        None => Cow::Borrowed(ctx.classified_ua),
    };
    let (country, region) = if let Some(c) = normalize_country(item.country.clone()) {
        (c, "(unknown)".to_string())
    } else if let Some((c, r)) = item_ip.and_then(|ip| state.geoip.resolve_ip(ip)) {
        (c, r)
    } else {
        (ctx.country.to_string(), ctx.region.to_string())
    };

    Ok(Some(QueuedEvent {
        site_id: site.site_id,
        ts_ms,
        candidates,
        event_kind,
        path: norm_url.path,
        props,
        ref_host: norm_ref.source,
        channel: norm_ref.channel,
        utm: norm_url.utm,
        country,
        region,
        browser: classified_ua.browser.clone(),
        os: classified_ua.os.clone(),
        device_type: classified_ua.device_type,
        viewport_class: ViewportClass::from_width(item.viewport_width),
    }))
}

fn normalize_batch_item(
    site: &Site,
    ctx: &BatchContext<'_>,
    item: &BatchEventItem,
    effective_ua: &str,
) -> Option<(
    i64,
    EventKind,
    NormalizedUrl,
    NormalizedReferrer,
    EventProps,
)> {
    if site.respect_gpc && ctx.gpc_header {
        return None;
    }
    let ua_opt = (!effective_ua.is_empty()).then_some(effective_ua);
    if matches!(
        classify_traffic(ua_opt, ctx.prefetch, false, item.allow_automated),
        BotFilterDecision::Drop(_)
    ) {
        return None;
    }
    let ts_ms = match item.ts.as_deref() {
        Some(ts) => DateTime::parse_from_rfc3339(ts.trim())
            .ok()?
            .timestamp_millis(),
        None => now_ms(),
    };
    let event_kind = EventKind::parse(item.event.as_deref().unwrap_or("pageview")).ok()?;
    let norm_url =
        normalize_url(&item.url, &site.allowed_query_keys, &site.allowed_origins).ok()?;
    if site.is_self_site && norm_url.path.starts_with("/v1/") {
        return None;
    }
    let norm_ref = normalize_referrer(
        item.referrer.as_deref(),
        &norm_url.host,
        &site.allowed_origins,
        &norm_url.utm,
    );
    let props = parse_props(item.props.as_ref()).ok()?;
    Some((ts_ms, event_kind, norm_url, norm_ref, props))
}
