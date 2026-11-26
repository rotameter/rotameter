#![forbid(unsafe_code)]
//! Arrow-aligned `DuckDB` OLAP DDL schema (`events`, `sessions`, and `rollups_*`).

use duckdb::Connection;
use rotameter_core::error::CoreError;

use crate::store::map_duck_err;

pub(crate) fn init_schema(conn: &Connection) -> Result<(), CoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS events (
           site_id        BLOB NOT NULL,
           ts_ms          BIGINT NOT NULL,
           session_id     BLOB NOT NULL,
           seq_in_session UINTEGER NOT NULL,
           visitor_id     BLOB NOT NULL,
           event_kind     VARCHAR NOT NULL,
           path           VARCHAR NOT NULL,
           props_json     VARCHAR
         );

         CREATE TABLE IF NOT EXISTS sessions (
           site_id         BLOB NOT NULL,
           session_id      BLOB NOT NULL,
           visitor_id      BLOB NOT NULL,
           started_at_ms   BIGINT NOT NULL,
           last_event_ms   BIGINT NOT NULL,
           event_count     UINTEGER NOT NULL,
           pageview_count  UINTEGER NOT NULL,
           entry_path      VARCHAR NOT NULL,
           exit_path       VARCHAR NOT NULL,
           ref_host        VARCHAR NOT NULL,
           channel         VARCHAR NOT NULL,
           utm_source      VARCHAR,
           utm_medium      VARCHAR,
           utm_campaign    VARCHAR,
           utm_content     VARCHAR,
           utm_term        VARCHAR,
           country         VARCHAR NOT NULL,
           region          VARCHAR NOT NULL,
           browser         VARCHAR NOT NULL,
           os              VARCHAR NOT NULL,
           device_type     UTINYINT NOT NULL,
           viewport_class  UTINYINT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS rollups_hourly (
           site_id           BLOB NOT NULL,
           bucket_ts_ms      BIGINT NOT NULL,
           pageviews         BIGINT NOT NULL,
           events_total      BIGINT NOT NULL,
           bounced_visits    BIGINT NOT NULL,
           visits_started    BIGINT NOT NULL,
           total_duration_ms BIGINT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS rollups_daily (
           site_id           BLOB NOT NULL,
           day_utc           INTEGER NOT NULL,
           pageviews         BIGINT NOT NULL,
           events_total      BIGINT NOT NULL,
           bounced_visits    BIGINT NOT NULL,
           visits_started    BIGINT NOT NULL,
           total_duration_ms BIGINT NOT NULL,
           visitors          BIGINT NOT NULL
         );

         CREATE TABLE IF NOT EXISTS rollups_path_daily (
           site_id           BLOB NOT NULL,
           day_utc           INTEGER NOT NULL,
           path              VARCHAR NOT NULL,
           pageviews         BIGINT NOT NULL,
           events_total      BIGINT NOT NULL,
           visits            BIGINT NOT NULL,
           visitors          BIGINT NOT NULL,
           bounced_visits    BIGINT NOT NULL,
           total_duration_ms BIGINT NOT NULL
         );",
    )
    .map_err(map_duck_err)
}
