#![forbid(unsafe_code)]
//! Integration tests for chunked `SQLite` control-plane and analytics exchange helpers.

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::utc_yyyymmdd;
use rotameter_core::query::{Metric, QueryFormat, QuerySpec, RangePreset, TimeRange};
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
use rotameter_core::types::{
    Channel, DeviceType, EventKind, EventProps, SaltRotation, Site, UtmParams, ViewportClass,
};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

fn seed_exchange_source(ts: i64) -> Result<(SqliteStore, Site, QuerySpec), CoreError> {
    let src = SqliteStore::open_in_memory(SqliteConfig::default(), ts)?;
    let org = src.ensure_default_org_sync(ts)?;
    let site_id = SiteId::from_uuidv7_parts(u64::try_from(ts).unwrap_or(0), [7; 10]);
    let site = Site {
        site_id,
        org_id: org.org_id,
        slug: "colinwolff.dev".to_string(),
        display_name: "Colin Wolff".to_string(),
        timezone: "UTC".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: true,
        is_self_site: false,
        allowed_origins: vec!["https://colinwolff.dev".to_string()],
        allowed_query_keys: vec!["tab".to_string()],
        raw_retention_days: Some(365),
        created_at_ms: ts,
        updated_at_ms: ts,
    };
    src.upsert_site_sync(&site)?;

    let day_utc = utc_yyyymmdd(ts);
    let hour_bucket_ms = ts - ts.rem_euclid(3_600_000);
    src.write_batch_sync(&WriteBatch {
        events: vec![
            IngestEvent {
                site_id,
                ts_ms: ts,
                session_id: [1; 16],
                seq_in_session: 1,
                visitor_id: [2; 16],
                event_kind: EventKind::Pageview,
                path: "/".to_string(),
                props: EventProps::from_pairs([("plan", "pro")])?,
            },
            IngestEvent {
                site_id,
                ts_ms: ts + 5_000,
                session_id: [1; 16],
                seq_in_session: 2,
                visitor_id: [2; 16],
                event_kind: EventKind::Custom("signup".to_string()),
                path: "/signup".to_string(),
                props: EventProps::default(),
            },
        ],
        sessions: vec![SessionUpsert {
            site_id,
            session_id: [1; 16],
            visitor_id: [2; 16],
            started_at_ms: ts,
            last_event_ms: ts + 5_000,
            event_count: 2,
            pageview_count: 1,
            entry_path: "/".to_string(),
            exit_path: "/signup".to_string(),
            ref_host: "news.ycombinator.com".to_string(),
            channel: Channel::Referral,
            utm: UtmParams {
                source: Some("hn".to_string()),
                medium: None,
                campaign: Some("launch".to_string()),
                content: None,
                term: None,
            },
            country: "US".to_string(),
            region: "CA".to_string(),
            browser: "Firefox 130".to_string(),
            os: "macOS 15".to_string(),
            device_type: DeviceType::Desktop,
            viewport_class: ViewportClass::Xl,
        }],
        rollups: vec![RollupDelta {
            site_id,
            hour_bucket_ms,
            day_utc,
            pageviews: 1,
            events_total: 2,
            bounced_visits: 0,
            visits_started: 1,
            total_duration_ms: 5_000,
        }],
    })?;

    let spec = QuerySpec {
        site_id: rotameter_core::id::SiteRef::parse(&site_id.to_string())?,
        range: TimeRange::Preset {
            preset: RangePreset::All,
        },
        granularity: None,
        metrics: vec![Metric::Visitors, Metric::Pageviews],
        group_by: vec![],
        filters: vec![],
        compare_previous: false,
        sort: None,
        limit: 50,
        offset: 0,
        format: QueryFormat::default(),
    };
    Ok((src, site, spec))
}

#[test]
fn chunked_exchange_roundtrips_control_and_analytics_between_stores() {
    let ts = 1_700_000_000_000_i64;
    let (src, site, spec) = seed_exchange_source(ts).unwrap();

    let ctrl = src.export_control_plane_sync().unwrap();
    let evs = src.read_events_chunk_sync(0, 100).unwrap();
    let sess = src.read_sessions_chunk_sync(0, 100).unwrap();
    let rh = src.read_rollups_hourly_chunk_sync(0, 100).unwrap();
    let rd = src.read_rollups_daily_chunk_sync(0, 100).unwrap();
    let rpd = src.read_rollups_path_daily_chunk_sync(0, 100).unwrap();

    assert_eq!(evs.len(), 2);
    assert_eq!(sess.len(), 1);
    assert_eq!(rh.len(), 1);
    assert_eq!(rd.len(), 1);
    assert_eq!(rpd.len(), 2);

    let dst = SqliteStore::open_in_memory(SqliteConfig::default(), ts + 10_000).unwrap();
    dst.import_control_plane_sync(&ctrl).unwrap();
    dst.import_events_chunk_sync(&evs).unwrap();
    dst.import_sessions_chunk_sync(&sess).unwrap();
    dst.import_rollups_hourly_chunk_sync(&rh).unwrap();
    dst.import_rollups_daily_chunk_sync(&rd).unwrap();
    dst.import_rollups_path_daily_chunk_sync(&rpd).unwrap();

    let q_src = src.query_sync(&site, &spec, ts + 10_000).unwrap();
    let q_dst = dst.query_sync(&site, &spec, ts + 10_000).unwrap();
    assert_eq!(q_src.summary, q_dst.summary);
}
