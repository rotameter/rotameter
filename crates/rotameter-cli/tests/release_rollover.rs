#![forbid(unsafe_code)]
//! Release gate: midnight UTC daily salt rollover with 30-minute session grace window.

#![cfg(feature = "sqlite")]

use chrono::DateTime;
use rotameter_core::id::{OrgId, SiteId, SiteRef};
use rotameter_core::privacy::{
    InMemorySaltProvider, resolve_visitor_candidates, utc_hour_floor_ms, utc_yyyymmdd,
};
use rotameter_core::query::{Metric, QueryFormat, QuerySpec, TimeRange};
use rotameter_core::session::{SessionDelta, SessionTracker};
use rotameter_core::storage::{
    AnalyticsQueryStore, ControlPlaneStore, EventIngestStore, IngestEvent, RollupDelta,
    SessionUpsert, WriteBatch,
};
use rotameter_core::types::{
    Channel, DeviceType, EventKind, EventProps, SaltRotation, Site, UtmParams, ViewportClass,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

fn rollover_site(org_id: OrgId, ts_ms: i64) -> Site {
    Site {
        site_id: SiteId::from_uuidv7_parts(u64::try_from(ts_ms.max(0)).unwrap_or(0), [9; 10]),
        org_id,
        slug: "rollover.dev".to_string(),
        display_name: "rollover.dev".to_string(),
        timezone: "UTC".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: Vec::new(),
        allowed_query_keys: Vec::new(),
        raw_retention_days: None,
        created_at_ms: ts_ms,
        updated_at_ms: ts_ms,
    }
}

fn build_rollover_batch(
    site_id: SiteId,
    t1: i64,
    t2: i64,
    d1: SessionDelta,
    d2: SessionDelta,
) -> WriteBatch {
    let pv = |ts_ms, d: SessionDelta, seq, path: &str| IngestEvent {
        site_id,
        ts_ms,
        session_id: d.session_id,
        seq_in_session: seq,
        visitor_id: d.visitor_id,
        event_kind: EventKind::Pageview,
        path: path.into(),
        props: EventProps::default(),
    };
    WriteBatch {
        events: vec![pv(t1, d1, 1, "/a"), pv(t2, d2, 2, "/b")],
        sessions: vec![SessionUpsert {
            site_id,
            session_id: d1.session_id,
            visitor_id: d1.visitor_id,
            started_at_ms: d2.started_at_ms,
            last_event_ms: d2.last_event_ms,
            event_count: 2,
            pageview_count: 2,
            entry_path: "/a".into(),
            exit_path: "/b".into(),
            ref_host: "(direct)".into(),
            channel: Channel::Direct,
            utm: UtmParams::default(),
            country: "US".into(),
            region: "OR".into(),
            browser: "Chrome".into(),
            os: "macOS".into(),
            device_type: DeviceType::Desktop,
            viewport_class: ViewportClass::Lg,
        }],
        rollups: vec![RollupDelta {
            site_id,
            hour_bucket_ms: utc_hour_floor_ms(t1),
            day_utc: utc_yyyymmdd(t1),
            pageviews: 2,
            events_total: 2,
            bounced_visits: 0,
            visits_started: 1,
            total_duration_ms: d2.duration_delta_ms,
        }],
    }
}

#[test]
fn midnight_salt_rollover_preserves_continuous_session() {
    let t1 = DateTime::parse_from_rfc3339("2026-11-22T23:55:00Z")
        .unwrap()
        .timestamp_millis();
    let t2 = DateTime::parse_from_rfc3339("2026-11-23T00:05:00Z")
        .unwrap()
        .timestamp_millis();
    let dir = tempfile::tempdir().unwrap();
    let store =
        SqliteStore::open(dir.path().join("rollover.db"), SqliteConfig::default(), t1).unwrap();
    let org = store.ensure_default_org(t1).unwrap();
    let site = store.upsert_site(&rollover_site(org.org_id, t1)).unwrap();

    let salts = InMemorySaltProvider::new(|w| {
        [u8::try_from(w.window_id.rem_euclid(200)).unwrap_or(1) + 1; 32]
    });
    let mut tracker = SessionTracker::with_defaults();
    let c1 = resolve_visitor_candidates(
        &salts,
        site.site_id,
        SaltRotation::Daily,
        t1,
        "203.0.113.1",
        "Mozilla/5.0",
    )
    .unwrap();
    let d1 = tracker.record_event(site.site_id, c1, t1, true, None, || [0x11; 16]);
    let c2 = resolve_visitor_candidates(
        &salts,
        site.site_id,
        SaltRotation::Daily,
        t2,
        "203.0.113.1",
        "Mozilla/5.0",
    )
    .unwrap();
    assert_ne!(c1.current, c2.current);
    assert_eq!(c2.previous, Some(c1.current));

    let d2 = tracker.record_event(site.site_id, c2, t2, true, None, || [0x22; 16]);
    assert_eq!(
        (d1.session_id, d1.visitor_id),
        (d2.session_id, d2.visitor_id)
    );
    store
        .write_batch(&build_rollover_batch(site.site_id, t1, t2, d1, d2))
        .unwrap();

    let res = store
        .query(
            &site,
            &QuerySpec {
                site_id: SiteRef::Id(site.site_id),
                range: TimeRange::Explicit {
                    from_ms: t1 - 60_000,
                    to_ms: t2 + 60_000,
                },
                granularity: None,
                metrics: vec![Metric::Visitors, Metric::Visits, Metric::Pageviews],
                group_by: Vec::new(),
                filters: Vec::new(),
                compare_previous: false,
                sort: None,
                limit: 10,
                offset: 0,
                format: QueryFormat::default(),
            },
            t2 + 60_000,
        )
        .unwrap();
    assert_eq!(
        (
            res.summary.visitors,
            res.summary.visits,
            res.summary.pageviews
        ),
        (1, 1, 2)
    );
}
