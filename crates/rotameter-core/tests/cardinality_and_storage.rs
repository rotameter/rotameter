#![forbid(unsafe_code)]
//! Integration tests for daily cardinality caps and pluggable `Storage` trait boundaries.

use std::collections::BTreeMap;

use rotameter_core::cardinality::{CardinalityGuard, CardinalityLimits, OVERFLOW_BUCKET};
use rotameter_core::id::SiteId;
use rotameter_core::privacy::{MS_PER_DAY, SaltProvider};
use rotameter_core::storage::{
    AnalyticsDataPlane, AnalyticsQueryStore, BreakdownRow, ControlPlaneStore, EventIngestStore,
    IngestEvent, KpiSummary, QueryMeta, QueryResult, RealtimePage, RealtimePulse, RealtimeSnapshot,
    RollupDelta, SessionUpsert, StorageCapabilities, TimeseriesPoint, WriteBatch,
};
use rotameter_core::types::{
    Channel, DeviceType, DimensionKind, EventKind, EventProps, UtmParams, ViewportClass,
};

#[test]
fn cardinality_guard_caps_novel_values_per_site_and_resets_daily() {
    let site_a = SiteId::from_uuidv7_parts(1_700_000_000_000, [1; 10]);
    let site_b = SiteId::from_uuidv7_parts(1_700_000_000_000, [2; 10]);
    let mut guard = CardinalityGuard::new(CardinalityLimits {
        paths_per_day: 2,
        referrers_per_day: 2,
        custom_values_per_day: 2,
    });

    let day_1 = 20_000 * MS_PER_DAY + 1_000;
    let day_2 = 20_001 * MS_PER_DAY + 1_000;

    assert_eq!(
        guard
            .check_or_record(site_a, DimensionKind::Path, day_1, "/a")
            .as_str(),
        "/a"
    );
    assert_eq!(
        guard
            .check_or_record(site_a, DimensionKind::Path, day_1, "/b")
            .as_str(),
        "/b"
    );
    // Repeated seen value stays accepted even when cap is full.
    assert_eq!(
        guard
            .check_or_record(site_a, DimensionKind::Path, day_1, "/a")
            .as_str(),
        "/a"
    );
    // Unseen 3rd path collapses into "(other)".
    let overflow = guard.check_or_record(site_a, DimensionKind::Path, day_1, "/c");
    assert!(overflow.is_overflowed());
    assert_eq!(overflow.as_str(), OVERFLOW_BUCKET);

    // Site B has an independent daily budget.
    assert_eq!(
        guard
            .check_or_record(site_b, DimensionKind::Path, day_1, "/c")
            .as_str(),
        "/c"
    );

    // Day 2 resets Site A's bucket.
    assert_eq!(
        guard
            .check_or_record(site_a, DimensionKind::Path, day_2, "/c")
            .as_str(),
        "/c"
    );
}

#[test]
fn write_batch_and_capabilities_round_trip() {
    fn _assert_object_safety(
        _: &dyn SaltProvider,
        _: &dyn ControlPlaneStore,
        _: &dyn EventIngestStore,
        _: &dyn AnalyticsQueryStore,
        _: &dyn AnalyticsDataPlane,
    ) {
    }

    let site_id = SiteId::from_uuidv7_parts(1_700_000_000_000, [5; 10]);
    let caps = StorageCapabilities {
        supports_raw_event_log: true,
        supports_arbitrary_distinct_visitors: true,
        supports_multi_node_concurrent_writes: false,
        supports_sql_funnels: false,
        supports_raw_append_only: false,
        supports_control_plane: true,
        supports_analytics_data_plane: true,
    };
    assert_eq!(
        serde_json::from_str::<StorageCapabilities>(&serde_json::to_string(&caps).unwrap())
            .unwrap(),
        caps
    );

    let batch = WriteBatch {
        events: vec![IngestEvent {
            site_id,
            ts_ms: 1_700_000_000_000,
            session_id: [1; 16],
            seq_in_session: 1,
            visitor_id: [2; 16],
            event_kind: EventKind::Pageview,
            path: "/".to_string(),
            props: EventProps::default(),
        }],
        sessions: vec![SessionUpsert {
            site_id,
            session_id: [1; 16],
            visitor_id: [2; 16],
            started_at_ms: 1_700_000_000_000,
            last_event_ms: 1_700_000_000_000,
            event_count: 1,
            pageview_count: 1,
            entry_path: "/".to_string(),
            exit_path: "/".to_string(),
            ref_host: "(direct)".to_string(),
            channel: Channel::Direct,
            utm: UtmParams::default(),
            country: "US".to_string(),
            region: "CA".to_string(),
            browser: "Chrome 131".to_string(),
            os: "macOS 15".to_string(),
            device_type: DeviceType::Desktop,
            viewport_class: ViewportClass::Xl,
        }],
        rollups: vec![RollupDelta {
            site_id,
            hour_bucket_ms: 1_699_999_200_000,
            day_utc: 20_231_114,
            pageviews: 1,
            events_total: 1,
            bounced_visits: 1,
            visits_started: 1,
            total_duration_ms: 0,
        }],
    };
    assert_eq!(
        serde_json::from_str::<WriteBatch>(&serde_json::to_string(&batch).unwrap()).unwrap(),
        batch
    );
}

#[test]
fn query_and_realtime_models_round_trip() {
    let site_id = SiteId::from_uuidv7_parts(1_700_000_000_000, [5; 10]);
    let result = QueryResult {
        site_id,
        slug: "colinwolff.dev".to_string(),
        timezone: "UTC".to_string(),
        window_from_ms: 100,
        window_to_ms: 200,
        summary: KpiSummary::default(),
        previous_summary: None,
        timeseries: vec![TimeseriesPoint {
            bucket_ms: 100,
            visitors: 1,
            pageviews: 1,
            visits: 1,
        }],
        rows: vec![BreakdownRow {
            key: BTreeMap::from([("path".to_string(), "/".to_string())]),
            visitors: 1,
            pageviews: 1,
            visits: 1,
            bounce_rate: 1.0,
            avg_visit_duration_ms: 0,
        }],
        meta: QueryMeta {
            total_rows: 1,
            query_ms: 1.5,
            truncated_by_cardinality_cap: false,
            watermark_ts: None,
            partial_nodes: None,
            served_by_node: None,
        },
    };
    assert_eq!(
        serde_json::from_str::<QueryResult>(&serde_json::to_string(&result).unwrap()).unwrap(),
        result
    );

    let rt = RealtimeSnapshot {
        site_id,
        slug: "colinwolff.dev".to_string(),
        active_visitors: 1,
        active_pages: vec![RealtimePage {
            path: "/".to_string(),
            active_visitors: 1,
        }],
        pulse: vec![RealtimePulse {
            minute_ms: 1_700_000_000_000,
            events: 1,
        }],
    };
    assert_eq!(
        serde_json::from_str::<RealtimeSnapshot>(&serde_json::to_string(&rt).unwrap()).unwrap(),
        rt
    );
}
