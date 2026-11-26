#![forbid(unsafe_code)]
//! Integration tests for breakdown sorting, pagination offsets, and `(other)` cardinality flags.

use rotameter_core::id::{OrgId, SiteId, SiteRef};
use rotameter_core::privacy::{MS_PER_HOUR, utc_yyyymmdd};
use rotameter_core::query::{
    Dimension, Metric, QueryFormat, QuerySort, QuerySpec, SortDir, TimeRange,
};
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
use rotameter_core::types::{
    Channel, DeviceType, EventKind, EventProps, SaltRotation, Site, UtmParams, ViewportClass,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

fn open_utc_site() -> (tempfile::TempDir, SqliteStore, Site) {
    let dir = tempfile::tempdir().unwrap_or_else(|_| std::process::abort());
    let store = SqliteStore::open(
        dir.path().join("b.db"),
        SqliteConfig::default(),
        1_700_000_000_000,
    )
    .unwrap_or_else(|_| std::process::abort());
    let org = store
        .ensure_default_org_sync(1_700_000_000_000)
        .unwrap_or_else(|_| std::process::abort());
    let site = Site {
        site_id: SiteId::from_uuidv7_parts(1_700_000_000_000, [3; 10]),
        org_id: OrgId::from_bytes(*org.org_id.as_bytes()),
        slug: "bd".to_string(),
        display_name: "BD".to_string(),
        timezone: "UTC".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: Vec::new(),
        allowed_query_keys: Vec::new(),
        raw_retention_days: None,
        created_at_ms: 1_700_000_000_000,
        updated_at_ms: 1_700_000_000_000,
    };
    let _ = store.upsert_site_sync(&site);
    (dir, store, site)
}

#[test]
fn breakdown_pagination_sorting_and_cardinality_flag() {
    let (_dir, store, site) = open_utc_site();
    let t0 = 1_705_300_000_000_i64;
    let items = [
        (1_u8, 10_000_i64, "/a", "US"),
        (2_u8, 0_i64, "/b", "CA"),
        (3_u8, 5_000_i64, "(other)", "DE"),
    ];
    let mut batch = WriteBatch::default();
    for (id, dur_ms, path, country) in items {
        let ts_ms = t0 + i64::from(id) * 1_000;
        batch.events.push(IngestEvent {
            site_id: site.site_id,
            ts_ms,
            session_id: [id; 16],
            seq_in_session: 1,
            visitor_id: [id; 16],
            event_kind: EventKind::Pageview,
            path: path.to_string(),
            props: EventProps::default(),
        });
        batch.sessions.push(SessionUpsert {
            site_id: site.site_id,
            session_id: [id; 16],
            visitor_id: [id; 16],
            started_at_ms: ts_ms,
            last_event_ms: ts_ms + dur_ms,
            event_count: 1,
            pageview_count: 1,
            entry_path: path.to_string(),
            exit_path: path.to_string(),
            ref_host: "(direct)".to_string(),
            channel: Channel::Direct,
            utm: UtmParams::default(),
            country: country.to_string(),
            region: "unknown".to_string(),
            browser: "Chrome".to_string(),
            os: "Linux".to_string(),
            device_type: DeviceType::Desktop,
            viewport_class: ViewportClass::Lg,
        });
        batch.rollups.push(RollupDelta {
            site_id: site.site_id,
            hour_bucket_ms: ts_ms.div_euclid(MS_PER_HOUR) * MS_PER_HOUR,
            day_utc: utc_yyyymmdd(ts_ms),
            pageviews: 1,
            events_total: 1,
            bounced_visits: 1,
            visits_started: 1,
            total_duration_ms: dur_ms,
        });
    }
    store.write_batch_sync(&batch).unwrap();

    let spec = QuerySpec {
        site_id: SiteRef::Id(site.site_id),
        range: TimeRange::Explicit {
            from_ms: t0,
            to_ms: t0 + 10_000,
        },
        granularity: None,
        metrics: vec![Metric::Visits],
        group_by: vec![Dimension::Path, Dimension::Country],
        filters: Vec::new(),
        compare_previous: false,
        sort: Some(QuerySort {
            by: Metric::AvgVisitDurationMs,
            dir: SortDir::Desc,
        }),
        limit: 2,
        offset: 1,
        format: QueryFormat::Json,
    };

    let res = store.query_sync(&site, &spec, t0 + 10_000).unwrap();
    assert_eq!(res.meta.total_rows, 3);
    assert_eq!(res.rows.len(), 2);
    assert!(res.meta.truncated_by_cardinality_cap);
    assert_eq!(
        res.rows[0].key.get("path").map(String::as_str),
        Some("(other)")
    );
    assert_eq!(res.rows[1].key.get("path").map(String::as_str), Some("/b"));
}
