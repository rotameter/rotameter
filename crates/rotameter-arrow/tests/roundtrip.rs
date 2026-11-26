#![forbid(unsafe_code)]
//! Round-trip Arrow Schema v1 codec tests for `rotameter-arrow`.

use std::collections::BTreeMap;

use rotameter_arrow::{
    arrow_to_query_result, arrow_to_write_batch, events_to_record_batch, query_result_to_arrow,
    record_batch_to_events, record_batch_to_rollups_daily, record_batch_to_rollups_hourly,
    record_batch_to_rollups_path_daily, record_batch_to_sessions, rollups_daily_to_record_batch,
    rollups_hourly_to_record_batch, rollups_path_daily_to_record_batch, sessions_to_record_batch,
    write_batch_to_arrow,
};
use rotameter_core::id::SiteId;
use rotameter_core::storage::{
    BreakdownRow, IngestEvent, KpiSummary, QueryMeta, QueryResult, RollupDailyRow, RollupDelta,
    RollupHourlyRow, RollupPathDailyRow, SessionUpsert, TimeseriesPoint, WriteBatch,
};
use rotameter_core::types::{Channel, DeviceType, EventKind, EventProps, UtmParams, ViewportClass};

fn sample_events_and_sessions(ts: i64, site_id: SiteId) -> (Vec<IngestEvent>, Vec<SessionUpsert>) {
    let events = vec![
        IngestEvent {
            site_id,
            ts_ms: ts,
            session_id: [1; 16],
            seq_in_session: 1,
            visitor_id: [2; 16],
            event_kind: EventKind::Pageview,
            path: "/".to_string(),
            props: EventProps::from_pairs([("plan", "pro")]).unwrap_or_default(),
        },
        IngestEvent {
            site_id,
            ts_ms: ts + 1_000,
            session_id: [1; 16],
            seq_in_session: 2,
            visitor_id: [2; 16],
            event_kind: EventKind::Custom("signup".to_string()),
            path: "/signup".to_string(),
            props: EventProps::default(),
        },
    ];
    let sessions = vec![SessionUpsert {
        site_id,
        session_id: [1; 16],
        visitor_id: [2; 16],
        started_at_ms: ts,
        last_event_ms: ts + 1_000,
        event_count: 2,
        pageview_count: 1,
        entry_path: "/".to_string(),
        exit_path: "/signup".to_string(),
        ref_host: "(direct)".to_string(),
        channel: Channel::Direct,
        utm: UtmParams {
            source: Some("hn".to_string()),
            medium: None,
            campaign: Some("launch".to_string()),
            content: Some("top".to_string()),
            term: None,
        },
        country: "US".to_string(),
        region: "OR".to_string(),
        browser: "Firefox 130".to_string(),
        os: "macOS 15".to_string(),
        device_type: DeviceType::Desktop,
        viewport_class: ViewportClass::Xl,
    }];
    (events, sessions)
}

#[test]
fn events_sessions_and_write_batch_roundtrip() {
    let ts = 1_700_000_000_000_i64;
    let site_id = SiteId::from_uuidv7_parts(1_700_000_000_000, [5; 10]);
    let (events, sessions) = sample_events_and_sessions(ts, site_id);

    let ev_batch = events_to_record_batch(&events).unwrap();
    assert_eq!(record_batch_to_events(&ev_batch).unwrap(), events);

    let sess_batch = sessions_to_record_batch(&sessions).unwrap();
    assert_eq!(record_batch_to_sessions(&sess_batch).unwrap(), sessions);

    let wb = WriteBatch {
        events,
        sessions,
        rollups: vec![RollupDelta {
            site_id,
            hour_bucket_ms: 1_699_999_200_000,
            day_utc: 20_231_114,
            pageviews: 1,
            events_total: 2,
            bounced_visits: 0,
            visits_started: 1,
            total_duration_ms: 1_000,
        }],
    };
    assert_eq!(
        arrow_to_write_batch(&write_batch_to_arrow(&wb).unwrap()).unwrap(),
        wb
    );
}

#[test]
fn rollups_and_query_result_roundtrip() {
    let ts = 1_700_000_000_000_i64;
    let site_id = SiteId::from_uuidv7_parts(1_700_000_000_000, [5; 10]);

    let hourly = vec![RollupHourlyRow {
        site_id,
        bucket_ts_ms: 1_699_999_200_000,
        pageviews: 1,
        events_total: 2,
        bounced_visits: 0,
        visits_started: 1,
        total_duration_ms: 1_000,
    }];
    assert_eq!(
        record_batch_to_rollups_hourly(&rollups_hourly_to_record_batch(&hourly).unwrap()).unwrap(),
        hourly
    );

    let daily = vec![RollupDailyRow {
        site_id,
        day_utc: 20_231_114,
        pageviews: 1,
        events_total: 2,
        bounced_visits: 0,
        visits_started: 1,
        total_duration_ms: 1_000,
        visitors: 1,
    }];
    assert_eq!(
        record_batch_to_rollups_daily(&rollups_daily_to_record_batch(&daily).unwrap()).unwrap(),
        daily
    );

    let path_daily = vec![RollupPathDailyRow {
        site_id,
        day_utc: 20_231_114,
        path: "/".to_string(),
        pageviews: 1,
        events_total: 1,
        visits: 1,
        visitors: 1,
        bounced_visits: 0,
        total_duration_ms: 1_000,
    }];
    let rpd_batch = rollups_path_daily_to_record_batch(&path_daily).unwrap();
    assert_eq!(
        record_batch_to_rollups_path_daily(&rpd_batch).unwrap(),
        path_daily
    );

    let qr = QueryResult {
        site_id,
        slug: "colinwolff.dev".to_string(),
        timezone: "UTC".to_string(),
        window_from_ms: ts,
        window_to_ms: ts + 86_400_000,
        summary: KpiSummary {
            visitors: 1,
            pageviews: 1,
            visits: 1,
            views_per_visit: 1.0,
            bounce_rate: 0.0,
            avg_visit_duration_ms: 1_000,
        },
        previous_summary: None,
        timeseries: vec![TimeseriesPoint {
            bucket_ms: ts,
            visitors: 1,
            pageviews: 1,
            visits: 1,
        }],
        rows: vec![BreakdownRow {
            key: BTreeMap::from([("path".to_string(), "/".to_string())]),
            visitors: 1,
            pageviews: 1,
            visits: 1,
            bounce_rate: 0.0,
            avg_visit_duration_ms: 1_000,
        }],
        meta: QueryMeta {
            total_rows: 1,
            query_ms: 1.5,
            truncated_by_cardinality_cap: false,
        },
    };
    assert_eq!(
        arrow_to_query_result(&query_result_to_arrow(&qr).unwrap()).unwrap(),
        qr
    );
}
