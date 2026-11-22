#![forbid(unsafe_code)]
//! Normalization and session/rollup folding helpers for `local_import`.

use std::collections::BTreeMap;

use chrono::DateTime;
use rotameter_client::types::BatchEventItem;
use rotameter_core::normalize::{
    NormalizedReferrer, NormalizedUrl, normalize_referrer, normalize_url,
};
use rotameter_core::privacy::{utc_hour_floor_ms, utc_yyyymmdd};
use rotameter_core::session::SessionDelta;
use rotameter_core::storage::{RollupDelta, SessionUpsert};
use rotameter_core::types::{
    Channel, DeviceType, EventKind, EventProps, Site, UtmParams, ViewportClass,
};

use crate::format::now_ms;
use crate::local_import::LocalFoldState;

pub(crate) fn record_session_and_rollup(
    ctx: &mut LocalFoldState<'_>,
    delta: &SessionDelta,
    path: String,
    ref_host: String,
    channel: Channel,
    viewport_width: Option<u16>,
    is_pv: bool,
) {
    let site_id = ctx.site.site_id;
    ctx.session_map
        .entry((site_id, delta.session_id))
        .and_modify(|s| {
            s.last_event_ms = s.last_event_ms.max(delta.last_event_ms);
            s.event_count = s.event_count.max(delta.event_count);
            s.pageview_count = s.pageview_count.max(delta.pageview_count);
            s.exit_path.clone_from(&path);
        })
        .or_insert_with(|| SessionUpsert {
            site_id,
            session_id: delta.session_id,
            visitor_id: delta.visitor_id,
            started_at_ms: delta.started_at_ms,
            last_event_ms: delta.last_event_ms,
            event_count: delta.event_count,
            pageview_count: delta.pageview_count,
            entry_path: path.clone(),
            exit_path: path,
            ref_host,
            channel,
            utm: UtmParams::default(),
            country: "(unknown)".to_string(),
            region: "(unknown)".to_string(),
            browser: "Other".to_string(),
            os: "Other".to_string(),
            device_type: DeviceType::Unknown,
            viewport_class: ViewportClass::from_width(viewport_width),
        });
    let hour = utc_hour_floor_ms(delta.started_at_ms);
    let r = ctx
        .rollup_map
        .entry((site_id, hour))
        .or_insert(RollupDelta {
            site_id,
            hour_bucket_ms: hour,
            day_utc: utc_yyyymmdd(delta.started_at_ms),
            pageviews: 0,
            events_total: 0,
            bounced_visits: 0,
            visits_started: 0,
            total_duration_ms: 0,
        });
    r.pageviews += i64::from(is_pv);
    r.events_total += 1;
    r.bounced_visits += i64::from(delta.bounce_delta);
    r.visits_started += i64::from(delta.is_new_session);
    r.total_duration_ms += delta.duration_delta_ms;
}

pub(crate) fn normalize_local_item(
    site: &Site,
    item: &BatchEventItem,
) -> Option<(
    i64,
    EventKind,
    NormalizedUrl,
    NormalizedReferrer,
    EventProps,
)> {
    let ts_ms = match item.ts.as_deref() {
        Some(ts) => DateTime::parse_from_rfc3339(ts.trim())
            .ok()?
            .timestamp_millis(),
        None => now_ms(),
    };
    let ek = EventKind::parse(item.event.as_deref().unwrap_or("pageview")).ok()?;
    let norm_url =
        normalize_url(&item.url, &site.allowed_query_keys, &site.allowed_origins).ok()?;
    let norm_ref = normalize_referrer(
        item.referrer.as_deref(),
        &norm_url.host,
        &site.allowed_origins,
        &norm_url.utm,
    );
    let props = parse_scalar_props(item.props.as_ref())?;
    Some((ts_ms, ek, norm_url, norm_ref, props))
}

fn parse_scalar_props(raw: Option<&BTreeMap<String, serde_json::Value>>) -> Option<EventProps> {
    let Some(map) = raw else {
        return Some(EventProps::default());
    };
    let mut pairs = Vec::with_capacity(map.len());
    for (k, v) in map {
        let s = match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => continue,
            _ => return None,
        };
        pairs.push((k.as_str(), s));
    }
    EventProps::from_pairs(pairs).ok()
}
