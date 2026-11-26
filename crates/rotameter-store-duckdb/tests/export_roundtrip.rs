#![forbid(unsafe_code)]
//! Integration roundtrip test: [`DuckDbStore`] portable Parquet export & restore parity.

use std::collections::BTreeMap;
use std::path::Path;

use rotameter_arrow::{
    events_to_record_batch, record_batch_to_events, record_batch_to_rollups_daily,
    record_batch_to_rollups_hourly, record_batch_to_rollups_path_daily, record_batch_to_sessions,
    rollups_daily_to_record_batch, rollups_hourly_to_record_batch,
    rollups_path_daily_to_record_batch, sessions_to_record_batch, stream_parquet_batches,
};
use rotameter_core::id::{OrgId, SiteId, SiteRef};
use rotameter_core::query::{
    Dimension, Granularity, Metric, QueryFormat, QuerySpec, RangePreset, TimeRange,
};
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
use rotameter_core::types::{
    Channel, DeviceType, EventKind, EventProps, SaltRotation, Site, UtmParams, ViewportClass,
};
use rotameter_store_duckdb::{DuckDbConfig, DuckDbStore};

fn sample_batch(site_id: SiteId, ts: i64) -> WriteBatch {
    WriteBatch {
        events: vec![IngestEvent {
            site_id,
            ts_ms: ts,
            session_id: [0x33; 16],
            seq_in_session: 1,
            visitor_id: [0x44; 16],
            event_kind: EventKind::Pageview,
            path: "/pricing".to_string(),
            props: EventProps::default(),
        }],
        sessions: vec![SessionUpsert {
            site_id,
            session_id: [0x33; 16],
            visitor_id: [0x44; 16],
            started_at_ms: ts,
            last_event_ms: ts,
            event_count: 1,
            pageview_count: 1,
            entry_path: "/pricing".to_string(),
            exit_path: "/pricing".to_string(),
            ref_host: "duckduckgo.com".to_string(),
            channel: Channel::OrganicSearch,
            utm: UtmParams::default(),
            country: "US".to_string(),
            region: "CA".to_string(),
            browser: "Firefox".to_string(),
            os: "Linux".to_string(),
            device_type: DeviceType::Desktop,
            viewport_class: ViewportClass::Xl,
        }],
        rollups: vec![RollupDelta {
            site_id,
            hour_bucket_ms: (ts / 3_600_000) * 3_600_000,
            day_utc: 20_241_126,
            pageviews: 1,
            events_total: 1,
            bounced_visits: 1,
            visits_started: 1,
            total_duration_ms: 0,
        }],
    }
}

fn import_dir_into_duckdb(
    dir: &Path,
    files: &BTreeMap<String, String>,
    dst: &DuckDbStore,
) -> Result<(), Box<dyn std::error::Error>> {
    for rel in files.keys() {
        let p = dir.join(rel);
        if rel.starts_with("events/") {
            stream_parquet_batches(&p, 1024, |rb| {
                let decoded = record_batch_to_events(&rb)?;
                dst.append_events_batch_sync(events_to_record_batch(&decoded)?)
                    .map_err(rotameter_arrow::ArrowError::Core)
            })?;
        } else if rel.starts_with("sessions/") {
            stream_parquet_batches(&p, 1024, |rb| {
                let decoded = record_batch_to_sessions(&rb)?;
                dst.append_sessions_batch_sync(sessions_to_record_batch(&decoded)?)
                    .map_err(rotameter_arrow::ArrowError::Core)
            })?;
        }
    }
    for (rel, table) in [
        ("rollups_hourly.parquet", "rollups_hourly"),
        ("rollups_daily.parquet", "rollups_daily"),
        ("rollups_path_daily.parquet", "rollups_path_daily"),
    ] {
        stream_parquet_batches(&dir.join(rel), 1024, |rb| {
            let batch = match table {
                "rollups_hourly" => {
                    rollups_hourly_to_record_batch(&record_batch_to_rollups_hourly(&rb)?)?
                }
                "rollups_daily" => {
                    rollups_daily_to_record_batch(&record_batch_to_rollups_daily(&rb)?)?
                }
                _ => rollups_path_daily_to_record_batch(&record_batch_to_rollups_path_daily(&rb)?)?,
            };
            dst.append_rollup_batch_sync(table, batch)
                .map_err(rotameter_arrow::ArrowError::Core)
        })?;
    }
    Ok(())
}

#[test]
fn duckdb_export_analytics_dir_roundtrips_into_fresh_store() {
    let site_id = SiteId::from_bytes([0x21; 16]);
    let ts = 1_732_580_000_000_i64;
    let site = Site {
        site_id,
        org_id: OrgId::from_bytes([0x01; 16]),
        slug: "wolff.industries".to_string(),
        display_name: "wolff.industries".to_string(),
        timezone: "UTC".to_string(),
        salt_rotation: SaltRotation::Daily,
        respect_gpc: false,
        is_self_site: false,
        allowed_origins: vec![],
        allowed_query_keys: vec![],
        raw_retention_days: Some(365),
        created_at_ms: ts,
        updated_at_ms: ts,
    };

    let src = DuckDbStore::open_in_memory(&DuckDbConfig::default()).unwrap();
    src.write_batch_sync(&sample_batch(site_id, ts)).unwrap();

    let dir = tempfile::tempdir().unwrap();
    let (files, counts) = src.export_analytics_dir_sync(dir.path()).unwrap();
    assert_eq!(counts.events, 1);
    assert_eq!(counts.sessions, 1);
    assert_eq!(counts.rollups_hourly, 1);
    assert_eq!(counts.rollups_daily, 1);
    assert!(!files.is_empty());

    let dst = DuckDbStore::open_in_memory(&DuckDbConfig::default()).unwrap();
    import_dir_into_duckdb(dir.path(), &files, &dst).unwrap();

    let spec = QuerySpec {
        site_id: SiteRef::Slug("wolff.industries".to_string()),
        range: TimeRange::Preset {
            preset: RangePreset::All,
        },
        granularity: Some(Granularity::Hour),
        metrics: vec![Metric::Visitors, Metric::Pageviews, Metric::Visits],
        group_by: vec![Dimension::Path],
        filters: vec![],
        compare_previous: false,
        sort: None,
        limit: 50,
        offset: 0,
        format: QueryFormat::default(),
    };
    let expected = src.query_sync(&site, &spec, ts + 5_000).unwrap();
    let actual = dst.query_sync(&site, &spec, ts + 5_000).unwrap();
    assert_eq!(actual.summary, expected.summary);
    assert_eq!(actual.timeseries, expected.timeseries);
    assert_eq!(actual.rows, expected.rows);
}
