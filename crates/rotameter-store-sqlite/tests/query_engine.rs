#![forbid(unsafe_code)]
//! Integration tests for `AnalyticsQueryStore` summary, previous window, and `IANA` timezone bucketing.

use rotameter_core::id::{OrgId, SiteId, SiteRef};
use rotameter_core::privacy::{MS_PER_DAY, MS_PER_HOUR, utc_yyyymmdd};
use rotameter_core::query::{
    Dimension, Granularity, Metric, QueryFormat, QuerySpec, RangePreset, TimeRange,
};
use rotameter_core::storage::{
    AnalyticsQueryStore, ControlPlaneStore, EventIngestStore, IngestEvent, RollupDelta,
    SessionUpsert, WriteBatch,
};
use rotameter_core::types::{
    Channel, DeviceType, EventKind, EventProps, SaltRotation, Site, UtmParams, ViewportClass,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

fn open_site(tz: &str) -> (tempfile::TempDir, SqliteStore, Site) {
    let dir = tempfile::tempdir().unwrap_or_else(|_| std::process::abort());
    let store = SqliteStore::open(
        dir.path().join("r.db"),
        SqliteConfig::default(),
        1_700_000_000_000,
    )
    .unwrap_or_else(|_| std::process::abort());
    let org = store
        .ensure_default_org(1_700_000_000_000)
        .unwrap_or_else(|_| std::process::abort());
    let site = Site {
        site_id: SiteId::from_uuidv7_parts(1_700_000_000_000, [9; 10]),
        org_id: OrgId::from_bytes(*org.org_id.as_bytes()),
        slug: "docs".to_string(),
        display_name: "Docs".to_string(),
        timezone: tz.to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: Vec::new(),
        allowed_query_keys: Vec::new(),
        raw_retention_days: None,
        created_at_ms: 1_700_000_000_000,
        updated_at_ms: 1_700_000_000_000,
    };
    let _ = store.upsert_site(&site);
    (dir, store, site)
}

fn push_hit(
    batch: &mut WriteBatch,
    site_id: SiteId,
    id: u8,
    ts_ms: i64,
    dur_ms: i64,
    path: &str,
    custom: bool,
) {
    let sess = [id; 16];
    let vis = [id.min(2); 16];
    let seq_cnt = if custom { 2 } else { 1 };
    batch.events.push(IngestEvent {
        site_id,
        ts_ms,
        session_id: sess,
        seq_in_session: 1,
        visitor_id: vis,
        event_kind: EventKind::Pageview,
        path: path.to_string(),
        props: EventProps::default(),
    });
    if custom {
        batch.events.push(IngestEvent {
            site_id,
            ts_ms: ts_ms + dur_ms,
            session_id: sess,
            seq_in_session: 2,
            visitor_id: vis,
            event_kind: EventKind::Custom("cta".to_string()),
            path: path.to_string(),
            props: EventProps::default(),
        });
    }
    batch.sessions.push(SessionUpsert {
        site_id,
        session_id: sess,
        visitor_id: vis,
        started_at_ms: ts_ms,
        last_event_ms: ts_ms + dur_ms,
        event_count: seq_cnt,
        pageview_count: 1,
        entry_path: path.to_string(),
        exit_path: path.to_string(),
        ref_host: "github.com".to_string(),
        channel: Channel::Referral,
        utm: UtmParams::default(),
        country: "US".to_string(),
        region: "CA".to_string(),
        browser: "Chrome".to_string(),
        os: "macOS".to_string(),
        device_type: DeviceType::Desktop,
        viewport_class: ViewportClass::Xl,
    });
    batch.rollups.push(RollupDelta {
        site_id,
        hour_bucket_ms: ts_ms.div_euclid(MS_PER_HOUR) * MS_PER_HOUR,
        day_utc: utc_yyyymmdd(ts_ms),
        pageviews: 1,
        events_total: i64::from(seq_cnt),
        bounced_visits: i64::from(!custom),
        visits_started: 1,
        total_duration_ms: dur_ms,
    });
}

#[test]
fn summary_previous_window_and_iana_timezone_day_bucketing() {
    let (_dir, store, site) = open_site("America/Los_Angeles");
    // 2024-01-15T06:00:00Z = 2024-01-14T22:00:00-08:00 (Los Angeles Jan 14)
    let jan14_la = 1_705_298_400_000_i64;
    let jan15_la = jan14_la + 12 * MS_PER_HOUR;

    let mut batch = WriteBatch::default();
    push_hit(&mut batch, site.site_id, 1, jan14_la, 0, "/", false);
    push_hit(&mut batch, site.site_id, 2, jan15_la, 40_000, "/docs", true);
    push_hit(
        &mut batch,
        site.site_id,
        3,
        jan15_la + 1_000,
        0,
        "/docs",
        false,
    );
    store.write_batch(&batch).unwrap();

    let spec = QuerySpec {
        site_id: SiteRef::Id(site.site_id),
        range: TimeRange::Explicit {
            from_ms: jan15_la - 6 * MS_PER_HOUR,
            to_ms: jan15_la + 6 * MS_PER_HOUR,
        },
        granularity: Some(Granularity::Day),
        metrics: vec![Metric::Visitors, Metric::Pageviews, Metric::Visits],
        group_by: vec![Dimension::Path],
        filters: Vec::new(),
        compare_previous: true,
        sort: None,
        limit: 50,
        offset: 0,
        format: QueryFormat::Json,
    };

    let res = store
        .query(&site, &spec, jan15_la + 6 * MS_PER_HOUR)
        .unwrap();
    assert_eq!(res.summary.visitors, 1);
    assert_eq!(res.summary.pageviews, 2);
    assert_eq!(res.summary.visits, 2);
    assert!((res.summary.bounce_rate - 0.5).abs() < 1e-6);
    assert_eq!(res.summary.avg_visit_duration_ms, 20_000);
    let prev = res.previous_summary.unwrap();
    assert_eq!(prev.visits, 1);
    assert!((prev.bounce_rate - 1.0).abs() < 1e-6);
    assert_eq!(res.rows.len(), 1);
    assert_eq!(
        res.rows[0].key.get("path").map(String::as_str),
        Some("/docs")
    );

    let full_spec = QuerySpec {
        range: TimeRange::Preset {
            preset: RangePreset::All,
        },
        ..spec
    };
    let full = store
        .query(&site, &full_spec, jan15_la + MS_PER_DAY)
        .unwrap();
    assert_eq!(full.timeseries.len(), 2);
    assert_eq!(full.timeseries[0].visits, 1);
    assert_eq!(full.timeseries[1].visits, 2);
}
