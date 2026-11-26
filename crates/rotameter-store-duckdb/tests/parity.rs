#![forbid(unsafe_code)]
//! Query-parity, realtime, and retention tests comparing `DuckDbStore` against `SqliteStore`.

use rotameter_core::error::CoreError;
use rotameter_core::id::{SiteId, SiteRef};
use rotameter_core::privacy::{MS_PER_HOUR, utc_yyyymmdd};
use rotameter_core::query::{
    Dimension, FilterOp, FilterValue, Granularity, Metric, QueryFilter, QueryFormat, QuerySpec,
    TimeRange,
};
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
use rotameter_core::types::{
    Channel, DeviceType, EventKind, EventProps, SaltRotation, Site, UtmParams, ViewportClass,
};
use rotameter_store_duckdb::{DuckDbConfig, DuckDbStore};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

fn sample_batch(site_id: SiteId, base_ms: i64) -> WriteBatch {
    let hour = base_ms - base_ms.rem_euclid(MS_PER_HOUR);
    WriteBatch {
        events: vec![
            IngestEvent {
                site_id,
                ts_ms: base_ms,
                session_id: [1; 16],
                seq_in_session: 1,
                visitor_id: [10; 16],
                event_kind: EventKind::Pageview,
                path: "/".to_string(),
                props: EventProps::default(),
            },
            IngestEvent {
                site_id,
                ts_ms: base_ms + 4_000,
                session_id: [1; 16],
                seq_in_session: 2,
                visitor_id: [10; 16],
                event_kind: EventKind::Pageview,
                path: "/docs".to_string(),
                props: EventProps::default(),
            },
            IngestEvent {
                site_id,
                ts_ms: base_ms + 10_000,
                session_id: [2; 16],
                seq_in_session: 1,
                visitor_id: [11; 16],
                event_kind: EventKind::Pageview,
                path: "/pricing".to_string(),
                props: EventProps::default(),
            },
        ],
        sessions: vec![
            SessionUpsert {
                site_id,
                session_id: [1; 16],
                visitor_id: [10; 16],
                started_at_ms: base_ms,
                last_event_ms: base_ms + 4_000,
                event_count: 2,
                pageview_count: 2,
                entry_path: "/".to_string(),
                exit_path: "/docs".to_string(),
                ref_host: "news.ycombinator.com".to_string(),
                channel: Channel::Referral,
                utm: UtmParams {
                    source: Some("hn".to_string()),
                    ..UtmParams::default()
                },
                country: "US".to_string(),
                region: "CA".to_string(),
                browser: "Firefox 130".to_string(),
                os: "macOS 15".to_string(),
                device_type: DeviceType::Desktop,
                viewport_class: ViewportClass::Xl,
            },
            SessionUpsert {
                site_id,
                session_id: [2; 16],
                visitor_id: [11; 16],
                started_at_ms: base_ms + 10_000,
                last_event_ms: base_ms + 10_000,
                event_count: 1,
                pageview_count: 1,
                entry_path: "/pricing".to_string(),
                exit_path: "/pricing".to_string(),
                ref_host: "(direct)".to_string(),
                channel: Channel::Direct,
                utm: UtmParams::default(),
                country: "DE".to_string(),
                region: "BE".to_string(),
                browser: "Chrome 130".to_string(),
                os: "Linux".to_string(),
                device_type: DeviceType::Mobile,
                viewport_class: ViewportClass::Sm,
            },
        ],
        rollups: vec![RollupDelta {
            site_id,
            hour_bucket_ms: hour,
            day_utc: utc_yyyymmdd(base_ms),
            pageviews: 3,
            events_total: 3,
            bounced_visits: 1,
            visits_started: 2,
            total_duration_ms: 4_000,
        }],
    }
}

fn seed_both(ts: i64) -> Result<(SqliteStore, DuckDbStore, Site, i64), CoreError> {
    let sql = SqliteStore::open_in_memory(SqliteConfig::default(), ts)?;
    let org = sql.ensure_default_org_sync(ts)?;
    let site_id = SiteId::from_uuidv7_parts(1_700_000_000_000, [3; 10]);
    let site = Site {
        site_id,
        org_id: org.org_id,
        slug: "colinwolff.dev".to_string(),
        display_name: "Colin Wolff".to_string(),
        timezone: "UTC".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: vec!["https://colinwolff.dev".to_string()],
        allowed_query_keys: vec![],
        raw_retention_days: Some(365),
        created_at_ms: ts,
        updated_at_ms: ts,
    };
    sql.upsert_site_sync(&site)?;

    let duck = DuckDbStore::open_in_memory(&DuckDbConfig::default())?;
    let batch = sample_batch(site_id, ts);
    sql.write_batch_sync(&batch)?;
    duck.write_batch_sync(&batch)?;
    Ok((sql, duck, site, ts + 20_000))
}

#[test]
fn duckdb_matches_sqlite_query_realtime_and_purge_parity() {
    let ts = 1_700_000_000_000_i64;
    let (sql, duck, site, now_ms) = seed_both(ts).unwrap();

    assert!(
        duck.lookup_active_session_sync(site.site_id, [10; 16], ts)
            .unwrap()
            .is_some()
    );

    let spec_unfiltered = QuerySpec {
        site_id: SiteRef::Slug("colinwolff.dev".to_string()),
        range: TimeRange::Explicit {
            from_ms: ts - 60_000,
            to_ms: now_ms,
        },
        granularity: Some(Granularity::Hour),
        metrics: vec![Metric::Visitors, Metric::Pageviews, Metric::Visits],
        group_by: vec![Dimension::Country, Dimension::Device],
        filters: vec![],
        compare_previous: true,
        sort: None,
        limit: 50,
        offset: 0,
        format: QueryFormat::default(),
    };
    let s_res = sql.query_sync(&site, &spec_unfiltered, now_ms).unwrap();
    let d_res = duck.query_sync(&site, &spec_unfiltered, now_ms).unwrap();
    assert_eq!(s_res.summary, d_res.summary);
    assert_eq!(s_res.previous_summary, d_res.previous_summary);
    assert_eq!(s_res.timeseries, d_res.timeseries);
    assert_eq!(s_res.rows, d_res.rows);

    let spec_path = QuerySpec {
        group_by: vec![Dimension::Path],
        filters: vec![QueryFilter {
            dimension: Dimension::Country,
            op: FilterOp::Eq,
            value: FilterValue::Scalar("US".to_string()),
        }],
        ..spec_unfiltered
    };
    let s_path = sql.query_sync(&site, &spec_path, now_ms).unwrap();
    let d_path = duck.query_sync(&site, &spec_path, now_ms).unwrap();
    assert_eq!(s_path.summary, d_path.summary);
    assert_eq!(s_path.timeseries, d_path.timeseries);
    assert_eq!(s_path.rows, d_path.rows);

    assert_eq!(
        sql.realtime_sync(&site, now_ms).unwrap().active_visitors,
        duck.realtime_sync(&site, now_ms).unwrap().active_visitors
    );
    assert!(
        duck.purge_before_sync(site.site_id, now_ms + 1_000)
            .unwrap()
            >= 5
    );
}
