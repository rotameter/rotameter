#![forbid(unsafe_code)]
//! Release gate: `30d` summary + timeseries + top-20 path query SLA (`p95 < 25 ms`).

#![cfg(feature = "sqlite")]

use std::time::Instant;

use rotameter_core::id::{SiteId, SiteRef};
use rotameter_core::privacy::{MS_PER_DAY, utc_hour_floor_ms, utc_yyyymmdd};
use rotameter_core::query::{
    Dimension, Granularity, Metric, QueryFormat, QuerySpec, RangePreset, TimeRange,
};
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
use rotameter_core::types::{
    Channel, DeviceType, EventKind, EventProps, SaltRotation, Site, UtmParams, ViewportClass,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

#[test]
fn query_sla_10k_events_p95_under_25ms() {
    let (_p50, p95, _p99) = seed_and_benchmark(10_000, 4).unwrap();
    let max_p95_ms = if cfg!(debug_assertions) { 100.0 } else { 25.0 };
    assert!(
        p95 < max_p95_ms,
        "10k-event 30d p95 {p95:.2}ms exceeded {max_p95_ms}ms"
    );
}

#[test]
#[ignore = "1M-event release gate executed via scripts/run-release-gates.sh in --release"]
fn query_sla_1m_events_p95_under_25ms() {
    let (p50, p95, p99) = seed_and_benchmark(1_000_000, 4).unwrap();
    eprintln!("1M-event 30d query SLA: p50={p50:.2}ms p95={p95:.2}ms p99={p99:.2}ms");
    assert!(p95 < 25.0, "1M-event 30d p95 {p95:.2}ms exceeded 25ms");
}

fn seed_and_benchmark(
    total_events: usize,
    events_per_session: usize,
) -> Result<(f64, f64, f64), Box<dyn std::error::Error>> {
    let now_ms = 1_795_370_000_000_i64;
    let dir = tempfile::tempdir()?;
    let store = SqliteStore::open(dir.path().join("sla.db"), SqliteConfig::default(), now_ms)?;
    let org = store.ensure_default_org_sync(now_ms)?;
    let site = store.upsert_site_sync(&Site {
        site_id: SiteId::from_uuidv7_parts(u64::try_from(now_ms).unwrap_or(0), [7; 10]),
        org_id: org.org_id,
        slug: "sla.dev".to_string(),
        display_name: "SLA".to_string(),
        timezone: "UTC".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: Vec::new(),
        allowed_query_keys: Vec::new(),
        raw_retention_days: None,
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
    })?;

    seed_events(
        &store,
        site.site_id,
        now_ms,
        total_events,
        events_per_session,
    )?;

    let spec = QuerySpec {
        site_id: SiteRef::Id(site.site_id),
        range: TimeRange::Preset {
            preset: RangePreset::Days30,
        },
        granularity: Some(Granularity::Day),
        metrics: vec![
            Metric::Visitors,
            Metric::Pageviews,
            Metric::Visits,
            Metric::ViewsPerVisit,
            Metric::BounceRate,
            Metric::AvgVisitDurationMs,
        ],
        group_by: vec![Dimension::Path],
        filters: Vec::new(),
        compare_previous: false,
        sort: None,
        limit: 20,
        offset: 0,
        format: QueryFormat::default(),
    };

    let _warmup = store.query_sync(&site, &spec, now_ms)?;
    let mut samples_ms = Vec::with_capacity(30);
    for _ in 0..30 {
        let t0 = Instant::now();
        let res = store.query_sync(&site, &spec, now_ms)?;
        assert_eq!(res.summary.pageviews, total_events as u64);
        samples_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
    }
    samples_ms.sort_by(f64::total_cmp);
    Ok((samples_ms[14], samples_ms[28], samples_ms[29]))
}

fn seed_events(
    store: &SqliteStore,
    site_id: SiteId,
    now_ms: i64,
    total_events: usize,
    events_per_session: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let sessions = total_events / events_per_session;
    let paths: Vec<String> = (0..100).map(|i| format!("/posts/article-{i}")).collect();
    let countries = ["US", "CA", "GB", "DE", "FR", "JP", "AU", "NL", "BR", "IN"];
    let mut batch = WriteBatch::default();

    for s in 0..sessions {
        let day_offset = i64::try_from(s % 29).unwrap_or(0);
        let ts = now_ms - day_offset * MS_PER_DAY - 60_000;
        let mut sid = [0_u8; 16];
        sid[..8].copy_from_slice(&s.to_be_bytes());
        let mut vid = [0_u8; 16];
        vid[..8].copy_from_slice(&(s % (sessions / 2).max(1)).to_be_bytes());
        vid[8..16].copy_from_slice(&day_offset.to_be_bytes());
        let p = &paths[s % paths.len()];

        batch.sessions.push(SessionUpsert {
            site_id,
            session_id: sid,
            visitor_id: vid,
            started_at_ms: ts,
            last_event_ms: ts + 30_000,
            event_count: u32::try_from(events_per_session).unwrap_or(4),
            pageview_count: u32::try_from(events_per_session).unwrap_or(4),
            entry_path: p.clone(),
            exit_path: paths[(s + 1) % paths.len()].clone(),
            ref_host: "news.ycombinator.com".to_string(),
            channel: Channel::Referral,
            utm: UtmParams::default(),
            country: countries[s % countries.len()].to_string(),
            region: "CA".to_string(),
            browser: "Chrome 131".to_string(),
            os: "macOS 15".to_string(),
            device_type: DeviceType::Desktop,
            viewport_class: ViewportClass::Lg,
        });
        for seq in 1..=events_per_session {
            batch.events.push(IngestEvent {
                site_id,
                ts_ms: ts + i64::try_from(seq).unwrap_or(1) * 1_000,
                session_id: sid,
                seq_in_session: u32::try_from(seq).unwrap_or(1),
                visitor_id: vid,
                event_kind: EventKind::Pageview,
                path: paths[(s + seq - 1) % paths.len()].clone(),
                props: EventProps::default(),
            });
        }
        batch.rollups.push(RollupDelta {
            site_id,
            hour_bucket_ms: utc_hour_floor_ms(ts),
            day_utc: utc_yyyymmdd(ts),
            pageviews: i64::try_from(events_per_session).unwrap_or(4),
            events_total: i64::try_from(events_per_session).unwrap_or(4),
            bounced_visits: 0,
            visits_started: 1,
            total_duration_ms: 30_000,
        });
        if batch.events.len() >= 10_000 {
            store.write_batch_sync(&batch)?;
            batch = WriteBatch::default();
        }
    }
    if !batch.events.is_empty() {
        store.write_batch_sync(&batch)?;
    }
    Ok(())
}
