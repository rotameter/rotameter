#![forbid(unsafe_code)]
//! Release gate: `100,000`-path cardinality bomb caps at `10,000` and collapses to `(other)`.

#![cfg(feature = "sqlite")]

use chrono::DateTime;
use rotameter_core::cardinality::{CardinalityGuard, CardinalityOutcome, OVERFLOW_BUCKET};
use rotameter_core::id::{OrgId, SiteId, SiteRef};
use rotameter_core::normalize::normalize_url;
use rotameter_core::query::{Dimension, Metric, QueryFormat, QuerySpec, TimeRange};
use rotameter_core::storage::{IngestEvent, SessionUpsert, WriteBatch};
use rotameter_core::types::{
    Channel, DeviceType, DimensionKind, EventKind, EventProps, SaltRotation, Site, UtmParams,
    ViewportClass,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

fn bomb_site(org_id: OrgId, ts_ms: i64) -> Site {
    Site {
        site_id: SiteId::from_uuidv7_parts(u64::try_from(ts_ms.max(0)).unwrap_or(0), [9; 10]),
        org_id,
        slug: "bomb.dev".to_string(),
        display_name: "bomb.dev".to_string(),
        timezone: "UTC".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: Vec::new(),
        allowed_query_keys: vec!["junk".into()],
        raw_retention_days: None,
        created_at_ms: ts_ms,
        updated_at_ms: ts_ms,
    }
}

fn push_path_hit(batch: &mut WriteBatch, site_id: SiteId, ts: i64, i: u32, path: &str) {
    let mut id16 = [0_u8; 16];
    id16[..4].copy_from_slice(&i.to_be_bytes());
    batch.events.push(IngestEvent {
        site_id,
        ts_ms: ts,
        session_id: id16,
        seq_in_session: 1,
        visitor_id: id16,
        event_kind: EventKind::Pageview,
        path: path.to_string(),
        props: EventProps::default(),
    });
    batch.sessions.push(SessionUpsert {
        site_id,
        session_id: id16,
        visitor_id: id16,
        started_at_ms: ts,
        last_event_ms: ts,
        event_count: 1,
        pageview_count: 1,
        entry_path: path.to_string(),
        exit_path: path.to_string(),
        ref_host: "(direct)".into(),
        channel: Channel::Direct,
        utm: UtmParams::default(),
        country: "(unknown)".into(),
        region: "(unknown)".into(),
        browser: "Other".into(),
        os: "Other".into(),
        device_type: DeviceType::Unknown,
        viewport_class: ViewportClass::Unknown,
    });
}

#[test]
fn cardinality_bomb_100k_paths_caps_at_10k_and_collapses_to_other() {
    let ts = DateTime::parse_from_rfc3339("2026-11-22T12:00:00Z")
        .unwrap()
        .timestamp_millis();
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteStore::open(dir.path().join("bomb.db"), SqliteConfig::default(), ts).unwrap();
    let org = store.ensure_default_org_sync(ts).unwrap();
    let site = store.upsert_site_sync(&bomb_site(org.org_id, ts)).unwrap();

    let mut guard = CardinalityGuard::with_defaults();
    let (mut accepted, mut overflowed) = (0_usize, 0_usize);
    let mut batch = WriteBatch::default();
    for i in 0..100_000_u32 {
        let norm = normalize_url(
            &format!("https://bomb.dev/?junk={i}"),
            &site.allowed_query_keys,
            &site.allowed_origins,
        )
        .unwrap();
        let outcome = guard.check_or_record(site.site_id, DimensionKind::Path, ts, &norm.path);
        match outcome {
            CardinalityOutcome::Accepted(_) => accepted += 1,
            CardinalityOutcome::Overflowed => overflowed += 1,
        }
        push_path_hit(&mut batch, site.site_id, ts, i, outcome.as_str());
        if batch.events.len() >= 5_000 {
            store.write_batch_sync(&batch).unwrap();
            batch.events.clear();
            batch.sessions.clear();
        }
    }
    if !batch.events.is_empty() {
        store.write_batch_sync(&batch).unwrap();
    }

    assert_eq!((accepted, overflowed), (10_000, 90_000));
    let res = store
        .query_sync(
            &site,
            &QuerySpec {
                site_id: SiteRef::Id(site.site_id),
                range: TimeRange::Explicit {
                    from_ms: ts - 60_000,
                    to_ms: ts + 60_000,
                },
                granularity: None,
                metrics: vec![Metric::Pageviews],
                group_by: vec![Dimension::Path],
                filters: Vec::new(),
                compare_previous: false,
                sort: None,
                limit: 5,
                offset: 0,
                format: QueryFormat::default(),
            },
            ts + 60_000,
        )
        .unwrap();
    assert_eq!(res.meta.total_rows, 10_001);
    assert_eq!(
        res.rows[0].key.get("path").map(String::as_str),
        Some(OVERFLOW_BUCKET)
    );
    assert_eq!(res.rows[0].pageviews, 90_000);
}
