#![forbid(unsafe_code)]
//! Integration tests for 5m realtime snapshot, 30m pulse, retention pruning, and site purge.

use rotameter_core::id::{OrgId, SiteId, SiteRef};
use rotameter_core::privacy::{MS_PER_DAY, MS_PER_HOUR, utc_yyyymmdd};
use rotameter_core::query::{Metric, QueryFormat, QuerySpec, TimeRange};
use rotameter_core::storage::{
    AnalyticsQueryStore, ControlPlaneStore, EventIngestStore, IngestEvent, RollupDelta,
    SessionUpsert, WriteBatch,
};
use rotameter_core::types::{
    Channel, DeviceType, EventKind, EventProps, SaltRotation, Site, UtmParams, ViewportClass,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

fn open_site(retention_days: Option<u32>, now_ms: i64) -> (tempfile::TempDir, SqliteStore, Site) {
    let Ok(dir) = tempfile::tempdir() else {
        std::process::abort()
    };
    let db_path = dir.path().join("rotameter.db");
    let Ok(store) = SqliteStore::open(&db_path, SqliteConfig::default(), now_ms) else {
        std::process::abort()
    };
    let Ok(org) = store.ensure_default_org(now_ms) else {
        std::process::abort()
    };
    let site = Site {
        site_id: SiteId::from_uuidv7_parts(u64::try_from(now_ms.max(0)).unwrap_or(0), [7; 10]),
        org_id: OrgId::from_bytes(*org.org_id.as_bytes()),
        slug: "app".to_string(),
        display_name: "App".to_string(),
        timezone: "UTC".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: Vec::new(),
        allowed_query_keys: Vec::new(),
        raw_retention_days: retention_days,
        created_at_ms: now_ms,
        updated_at_ms: now_ms,
    };
    let _ = store.upsert_site(&site);
    (dir, store, site)
}

fn add_hit(batch: &mut WriteBatch, site_id: SiteId, id: u8, vis: u8, ts_ms: i64, path: &str) {
    batch.events.push(IngestEvent {
        site_id,
        ts_ms,
        session_id: [id; 16],
        seq_in_session: 1,
        visitor_id: [vis; 16],
        event_kind: EventKind::Pageview,
        path: path.to_string(),
        props: EventProps::default(),
    });
    batch.sessions.push(SessionUpsert {
        site_id,
        session_id: [id; 16],
        visitor_id: [vis; 16],
        started_at_ms: ts_ms,
        last_event_ms: ts_ms,
        event_count: 1,
        pageview_count: 1,
        entry_path: path.to_string(),
        exit_path: path.to_string(),
        ref_host: "(direct)".to_string(),
        channel: Channel::Direct,
        utm: UtmParams::default(),
        country: "US".to_string(),
        region: "unknown".to_string(),
        browser: "Chrome".to_string(),
        os: "Linux".to_string(),
        device_type: DeviceType::Desktop,
        viewport_class: ViewportClass::Lg,
    });
    batch.rollups.push(RollupDelta {
        site_id,
        hour_bucket_ms: ts_ms.div_euclid(MS_PER_HOUR) * MS_PER_HOUR,
        day_utc: utc_yyyymmdd(ts_ms),
        pageviews: 1,
        events_total: 1,
        bounced_visits: 1,
        visits_started: 1,
        total_duration_ms: 0,
    });
}

#[test]
fn realtime_five_minute_cutoff_and_thirty_minute_pulse() {
    let now_ms = 1_705_305_600_000_i64;
    let (_dir, store, site) = open_site(Some(7), now_ms);
    let mut batch = WriteBatch::default();
    // 10m ago: outside 5m active window, inside 30m pulse.
    add_hit(&mut batch, site.site_id, 1, 1, now_ms - 10 * 60_000, "/old");
    // 2m ago and now: inside 5m active window.
    add_hit(&mut batch, site.site_id, 2, 2, now_ms - 2 * 60_000, "/home");
    add_hit(&mut batch, site.site_id, 3, 2, now_ms - 60_000, "/pricing");
    add_hit(&mut batch, site.site_id, 4, 3, now_ms, "/home");
    store.write_batch(&batch).unwrap();

    let rt = store.realtime(&site, now_ms).unwrap();
    assert_eq!(rt.active_visitors, 2);
    assert_eq!(rt.active_pages.len(), 2);
    assert_eq!(rt.active_pages[0].path, "/home");
    assert_eq!(rt.active_pages[0].active_visitors, 2);
    assert_eq!(rt.pulse.len(), 30);
    assert_eq!(rt.pulse[29].minute_ms, now_ms);
    assert_eq!(rt.pulse[29].events, 1);
    assert_eq!(rt.pulse.iter().map(|p| p.events).sum::<u64>(), 4);
}

#[test]
fn retention_pruner_preserves_rollups_and_purge_before_deletes_all() {
    let now_ms = 1_705_363_200_000_i64;
    let (_dir, store, site) = open_site(Some(7), now_ms);
    let old_ts = now_ms - 20 * MS_PER_DAY;
    let recent_ts = now_ms - MS_PER_HOUR;

    let mut batch = WriteBatch::default();
    add_hit(&mut batch, site.site_id, 1, 1, old_ts, "/legacy");
    add_hit(&mut batch, site.site_id, 2, 2, recent_ts, "/fresh");
    store.write_batch(&batch).unwrap();

    let pruned = store.prune_expired_sites(now_ms).unwrap();
    assert_eq!(pruned, 2); // 1 old event + 1 old session

    let spec = QuerySpec {
        site_id: SiteRef::Id(site.site_id),
        range: TimeRange::Explicit {
            from_ms: old_ts - MS_PER_HOUR,
            to_ms: now_ms + MS_PER_HOUR,
        },
        granularity: None,
        metrics: vec![Metric::Pageviews, Metric::Visits],
        group_by: Vec::new(),
        filters: Vec::new(),
        compare_previous: false,
        sort: None,
        limit: 50,
        offset: 0,
        format: QueryFormat::Json,
    };
    let post_prune = store.query(&site, &spec, now_ms).unwrap();
    assert_eq!(post_prune.summary.pageviews, 2);
    assert_eq!(post_prune.summary.visits, 2);

    let purged = store.purge_before(site.site_id, recent_ts).unwrap();
    assert!(purged >= 2); // hourly + daily rollup removed
    let post_purge = store.query(&site, &spec, now_ms).unwrap();
    assert_eq!(post_purge.summary.pageviews, 1);
    assert_eq!(post_purge.summary.visits, 1);
}
