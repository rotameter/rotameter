#![forbid(unsafe_code)]
//! Canonical Arrow Schema v1 definitions for Rotameter exchange tables and query results.

use std::collections::HashMap;
use std::sync::Arc;

use arrow_schema::{DataType, Field, Schema, SchemaRef};

/// Canonical exchange Arrow schema version (`v1`).
pub const ARROW_SCHEMA_VERSION: u32 = 1;

fn versioned_schema(table: &str, fields: Vec<Field>) -> SchemaRef {
    let mut meta = HashMap::new();
    meta.insert(
        "rotameter.arrow_schema_version".to_string(),
        ARROW_SCHEMA_VERSION.to_string(),
    );
    meta.insert("rotameter.table".to_string(), table.to_string());
    Arc::new(Schema::new_with_metadata(fields, meta))
}

/// Arrow Schema v1 for raw ordered `events` (`IngestEvent`).
pub fn events_schema() -> SchemaRef {
    versioned_schema(
        "events",
        vec![
            Field::new("site_id", DataType::FixedSizeBinary(16), false),
            Field::new("ts_ms", DataType::Int64, false),
            Field::new("session_id", DataType::FixedSizeBinary(16), false),
            Field::new("seq_in_session", DataType::UInt32, false),
            Field::new("visitor_id", DataType::FixedSizeBinary(16), false),
            Field::new("event_kind", DataType::Utf8, false),
            Field::new("path", DataType::Utf8, false),
            Field::new("props_json", DataType::Utf8, true),
        ],
    )
}

/// Arrow Schema v1 for 30-minute sliding `sessions` (`SessionUpsert`).
pub fn sessions_schema() -> SchemaRef {
    versioned_schema(
        "sessions",
        vec![
            Field::new("site_id", DataType::FixedSizeBinary(16), false),
            Field::new("session_id", DataType::FixedSizeBinary(16), false),
            Field::new("visitor_id", DataType::FixedSizeBinary(16), false),
            Field::new("started_at_ms", DataType::Int64, false),
            Field::new("last_event_ms", DataType::Int64, false),
            Field::new("event_count", DataType::UInt32, false),
            Field::new("pageview_count", DataType::UInt32, false),
            Field::new("entry_path", DataType::Utf8, false),
            Field::new("exit_path", DataType::Utf8, false),
            Field::new("ref_host", DataType::Utf8, false),
            Field::new("channel", DataType::Utf8, false),
            Field::new("utm_source", DataType::Utf8, true),
            Field::new("utm_medium", DataType::Utf8, true),
            Field::new("utm_campaign", DataType::Utf8, true),
            Field::new("utm_content", DataType::Utf8, true),
            Field::new("utm_term", DataType::Utf8, true),
            Field::new("country", DataType::Utf8, false),
            Field::new("region", DataType::Utf8, false),
            Field::new("browser", DataType::Utf8, false),
            Field::new("os", DataType::Utf8, false),
            Field::new("device_type", DataType::UInt8, false),
            Field::new("viewport_class", DataType::UInt8, false),
        ],
    )
}

/// Arrow Schema v1 for pre-aggregated `rollups_hourly` rows.
pub fn rollups_hourly_schema() -> SchemaRef {
    versioned_schema(
        "rollups_hourly",
        vec![
            Field::new("site_id", DataType::FixedSizeBinary(16), false),
            Field::new("bucket_ts_ms", DataType::Int64, false),
            Field::new("pageviews", DataType::Int64, false),
            Field::new("events_total", DataType::Int64, false),
            Field::new("bounced_visits", DataType::Int64, false),
            Field::new("visits_started", DataType::Int64, false),
            Field::new("total_duration_ms", DataType::Int64, false),
        ],
    )
}

/// Arrow Schema v1 for pre-aggregated `rollups_daily` rows.
pub fn rollups_daily_schema() -> SchemaRef {
    versioned_schema(
        "rollups_daily",
        vec![
            Field::new("site_id", DataType::FixedSizeBinary(16), false),
            Field::new("day_utc", DataType::Int32, false),
            Field::new("pageviews", DataType::Int64, false),
            Field::new("events_total", DataType::Int64, false),
            Field::new("bounced_visits", DataType::Int64, false),
            Field::new("visits_started", DataType::Int64, false),
            Field::new("total_duration_ms", DataType::Int64, false),
            Field::new("visitors", DataType::Int64, false),
        ],
    )
}

/// Arrow Schema v1 for pre-aggregated `rollups_path_daily` rows.
pub fn rollups_path_daily_schema() -> SchemaRef {
    versioned_schema(
        "rollups_path_daily",
        vec![
            Field::new("site_id", DataType::FixedSizeBinary(16), false),
            Field::new("day_utc", DataType::Int32, false),
            Field::new("path", DataType::Utf8, false),
            Field::new("pageviews", DataType::Int64, false),
            Field::new("events_total", DataType::Int64, false),
            Field::new("visits", DataType::Int64, false),
            Field::new("visitors", DataType::Int64, false),
            Field::new("bounced_visits", DataType::Int64, false),
            Field::new("total_duration_ms", DataType::Int64, false),
        ],
    )
}

/// Arrow Schema v1 for `QueryResult.timeseries`.
pub fn query_timeseries_schema() -> SchemaRef {
    versioned_schema(
        "query_timeseries",
        vec![
            Field::new("bucket_ms", DataType::Int64, false),
            Field::new("visitors", DataType::UInt64, false),
            Field::new("pageviews", DataType::UInt64, false),
            Field::new("visits", DataType::UInt64, false),
        ],
    )
}

/// Arrow Schema v1 for `QueryResult.rows` breakdowns.
pub fn query_rows_schema() -> SchemaRef {
    versioned_schema(
        "query_rows",
        vec![
            Field::new("key_json", DataType::Utf8, false),
            Field::new("visitors", DataType::UInt64, false),
            Field::new("pageviews", DataType::UInt64, false),
            Field::new("visits", DataType::UInt64, false),
            Field::new("bounce_rate", DataType::Float64, false),
            Field::new("avg_visit_duration_ms", DataType::UInt64, false),
        ],
    )
}
