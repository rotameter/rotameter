#![forbid(unsafe_code)]
//! Single-session event sequence, session upsert, and rollup delta synthesis.

use rotameter_core::privacy::{utc_hour_floor_ms, utc_yyyymmdd};
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
use rotameter_core::types::{EventKind, EventProps, Site, UtmParams};

use crate::seed_catalog::{CLIENTS, GEOS, PATHS, SOURCES};
use crate::seed_gen::SeedStats;
use crate::seed_rng::SeedRng;

pub(crate) fn emit_session(
    site: &Site,
    visitor_id: [u8; 16],
    started_at_ms: i64,
    rng: &mut SeedRng,
    batch: &mut WriteBatch,
    stats: &mut SeedStats,
) {
    let max_last_ms = started_at_ms.saturating_add(180_000);
    emit_capped_session(
        site,
        visitor_id,
        started_at_ms,
        max_last_ms,
        rng,
        batch,
        stats,
    );
}

pub(crate) fn emit_capped_session(
    site: &Site,
    visitor_id: [u8; 16],
    started_at_ms: i64,
    max_ts_ms: i64,
    rng: &mut SeedRng,
    batch: &mut WriteBatch,
    stats: &mut SeedStats,
) {
    let session_id = rng.next_bytes16();
    let roll = rng.range_usize(100);
    let event_count: u32 = if roll < 44 {
        1
    } else if roll < 76 {
        2
    } else if roll < 92 {
        3
    } else {
        4
    };
    let entry_path = (*rng.pick(PATHS)).to_string();
    let mut exit_path = entry_path.clone();
    let mut ts = started_at_ms;
    let mut pageviews = 0_u32;

    for seq in 1..=event_count {
        let path = if seq == 1 {
            entry_path.clone()
        } else {
            (*rng.pick(PATHS)).to_string()
        };
        let event_kind = pick_event_kind(seq, &path, rng);
        if event_kind == EventKind::Pageview {
            pageviews += 1;
        }
        if seq == event_count {
            exit_path.clone_from(&path);
        }
        batch.events.push(IngestEvent {
            site_id: site.site_id,
            ts_ms: ts,
            session_id,
            seq_in_session: seq,
            visitor_id,
            event_kind,
            path,
            props: EventProps::default(),
        });
        ts = ts
            .saturating_add(rng.range_i64(4_000, 45_000))
            .min(max_ts_ms);
    }

    let last_event_ms = batch.events.last().map_or(started_at_ms, |e| e.ts_ms);
    let (ref_host, channel, us, um, uc) = *rng.pick(SOURCES);
    let (country, region) = *rng.pick(GEOS);
    let (browser, os, device_type, viewport_class) = *rng.pick(CLIENTS);
    batch.sessions.push(SessionUpsert {
        site_id: site.site_id,
        session_id,
        visitor_id,
        started_at_ms,
        last_event_ms,
        event_count,
        pageview_count: pageviews,
        entry_path,
        exit_path,
        ref_host: ref_host.to_string(),
        channel,
        utm: UtmParams {
            source: us.map(ToString::to_string),
            medium: um.map(ToString::to_string),
            campaign: uc.map(ToString::to_string),
            content: None,
            term: None,
        },
        country: country.to_string(),
        region: region.to_string(),
        browser: browser.to_string(),
        os: os.to_string(),
        device_type,
        viewport_class,
    });
    batch.rollups.push(RollupDelta {
        site_id: site.site_id,
        hour_bucket_ms: utc_hour_floor_ms(started_at_ms),
        day_utc: utc_yyyymmdd(started_at_ms),
        pageviews: i64::from(pageviews),
        events_total: i64::from(event_count),
        bounced_visits: i64::from(event_count <= 1),
        visits_started: 1,
        total_duration_ms: last_event_ms.saturating_sub(started_at_ms),
    });
    stats.sessions += 1;
    stats.events += u64::from(event_count);
}

fn pick_event_kind(seq: u32, path: &str, rng: &mut SeedRng) -> EventKind {
    if path == "/404" {
        EventKind::NotFound
    } else if seq > 1 && rng.range_usize(100) < 12 {
        EventKind::OutboundClick
    } else if seq > 2 && rng.range_usize(100) < 10 {
        EventKind::Custom("signup_click".to_string())
    } else {
        EventKind::Pageview
    }
}
