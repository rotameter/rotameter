#![forbid(unsafe_code)]
//! Chunked deduplicated `sessions` reader for portable Arrow/Parquet export from [`DuckDbStore`].

use duckdb::{Row, params};
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::storage::SessionUpsert;
use rotameter_core::types::{Channel, DeviceType, UtmParams, ViewportClass};

use crate::export_read::{bad_col, blob_col, i64_lim};
use crate::store::{DuckDbStore, map_duck_err};

impl DuckDbStore {
    /// Reads deduplicated `sessions` rows ordered by `(site_id, session_id)`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query or decode failure.
    pub fn read_sessions_chunk_sync(
        &self,
        offset: usize,
        limit: usize,
    ) -> Result<Vec<SessionUpsert>, CoreError> {
        self.with_conn(|conn| {
            let mut stmt = conn
                .prepare(
                    "WITH dedup AS (
                       SELECT *, ROW_NUMBER() OVER (
                         PARTITION BY site_id, session_id
                         ORDER BY last_event_ms DESC, event_count DESC
                       ) AS rn
                       FROM sessions
                     )
                     SELECT site_id, session_id, visitor_id, started_at_ms, last_event_ms,
                            CAST(event_count AS BIGINT), CAST(pageview_count AS BIGINT),
                            entry_path, exit_path, ref_host, channel,
                            COALESCE(utm_source, ''), COALESCE(utm_medium, ''),
                            COALESCE(utm_campaign, ''), COALESCE(utm_content, ''),
                            COALESCE(utm_term, ''),
                            country, region, browser, os,
                            CAST(device_type AS BIGINT), CAST(viewport_class AS BIGINT)
                     FROM dedup
                     WHERE rn = 1
                     ORDER BY site_id, session_id
                     LIMIT ?1 OFFSET ?2",
                )
                .map_err(map_duck_err)?;
            let rows = stmt
                .query_map(params![i64_lim(limit), i64_lim(offset)], decode_session)
                .map_err(map_duck_err)?;
            rows.collect::<Result<Vec<_>, _>>().map_err(map_duck_err)
        })
    }
}

fn decode_session(r: &Row<'_>) -> duckdb::Result<SessionUpsert> {
    let ec_raw: i64 = r.get(5)?;
    let pvc_raw: i64 = r.get(6)?;
    let ch_raw: String = r.get(10)?;
    let channel = Channel::parse(&ch_raw).ok_or_else(|| bad_col(10))?;
    let dev_raw: i64 = r.get(20)?;
    let vp_raw: i64 = r.get(21)?;
    Ok(SessionUpsert {
        site_id: SiteId::from_bytes(blob_col::<16>(r, 0)?),
        session_id: blob_col::<16>(r, 1)?,
        visitor_id: blob_col::<16>(r, 2)?,
        started_at_ms: r.get(3)?,
        last_event_ms: r.get(4)?,
        event_count: u32::try_from(ec_raw.max(0)).map_err(|_| bad_col(5))?,
        pageview_count: u32::try_from(pvc_raw.max(0)).map_err(|_| bad_col(6))?,
        entry_path: r.get(7)?,
        exit_path: r.get(8)?,
        ref_host: r.get(9)?,
        channel,
        utm: UtmParams {
            source: non_empty(r.get(11)?),
            medium: non_empty(r.get(12)?),
            campaign: non_empty(r.get(13)?),
            content: non_empty(r.get(14)?),
            term: non_empty(r.get(15)?),
        },
        country: r.get(16)?,
        region: r.get(17)?,
        browser: r.get(18)?,
        os: r.get(19)?,
        device_type: DeviceType::from_u8(u8::try_from(dev_raw.max(0)).unwrap_or(0)),
        viewport_class: ViewportClass::from_u8(u8::try_from(vp_raw.max(0)).unwrap_or(0)),
    })
}

fn non_empty(val: String) -> Option<String> {
    (!val.is_empty()).then_some(val)
}
