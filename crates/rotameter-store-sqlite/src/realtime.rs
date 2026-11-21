#![forbid(unsafe_code)]
//! Trailing 5-minute active visitor/page snapshot and 30-minute pulse strip queries.

use std::collections::HashMap;

use rotameter_core::error::CoreError;
use rotameter_core::storage::{RealtimePage, RealtimePulse, RealtimeSnapshot};
use rotameter_core::types::Site;
use rusqlite::{Connection, params};

use crate::config::map_sql_err;
use crate::query::non_neg_u64;

const REALTIME_WINDOW_MS: i64 = 5 * 60_000;
const MS_PER_MINUTE: i64 = 60_000;
const PULSE_MINUTES: i64 = 30;
const MAX_ACTIVE_PAGES: i64 = 10;

pub(crate) fn query_realtime(
    conn: &Connection,
    site: &Site,
    now_ms: i64,
) -> Result<RealtimeSnapshot, CoreError> {
    let site_blob = site.site_id.as_bytes().as_slice();
    let active_cutoff = now_ms.saturating_sub(REALTIME_WINDOW_MS);

    let active_visitors: i64 = conn
        .query_row(
            "SELECT COUNT(DISTINCT visitor_id)
             FROM sessions
             WHERE site_id = ?1 AND last_event_ms >= ?2 AND last_event_ms <= ?3",
            params![site_blob, active_cutoff, now_ms],
            |r| r.get(0),
        )
        .map_err(map_sql_err)?;

    let active_pages = query_active_pages(conn, site_blob, active_cutoff, now_ms)?;
    let pulse = query_pulse_strip(conn, site_blob, now_ms)?;

    Ok(RealtimeSnapshot {
        site_id: site.site_id,
        slug: site.slug.clone(),
        active_visitors: non_neg_u64(active_visitors),
        active_pages,
        pulse,
    })
}

fn query_active_pages(
    conn: &Connection,
    site_blob: &[u8],
    active_cutoff: i64,
    now_ms: i64,
) -> Result<Vec<RealtimePage>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT d.value, COUNT(DISTINCT e.visitor_id) AS visitors
             FROM events e
             JOIN dim_strings d ON d.id = e.path_id
             WHERE e.site_id = ?1 AND e.ts_ms >= ?2 AND e.ts_ms <= ?3
             GROUP BY d.value
             ORDER BY visitors DESC, d.value ASC
             LIMIT ?4",
        )
        .map_err(map_sql_err)?;
    let mut rows = stmt
        .query(params![site_blob, active_cutoff, now_ms, MAX_ACTIVE_PAGES])
        .map_err(map_sql_err)?;
    let mut pages = Vec::new();
    while let Some(row) = rows.next().map_err(map_sql_err)? {
        let path: String = row.get(0).map_err(map_sql_err)?;
        let count: i64 = row.get(1).map_err(map_sql_err)?;
        pages.push(RealtimePage {
            path,
            active_visitors: non_neg_u64(count),
        });
    }
    Ok(pages)
}

fn query_pulse_strip(
    conn: &Connection,
    site_blob: &[u8],
    now_ms: i64,
) -> Result<Vec<RealtimePulse>, CoreError> {
    let end_min = now_ms.div_euclid(MS_PER_MINUTE) * MS_PER_MINUTE;
    let start_min = end_min - (PULSE_MINUTES - 1) * MS_PER_MINUTE;

    let mut counts: HashMap<i64, u64> = HashMap::new();
    {
        let mut stmt = conn
            .prepare(
                "SELECT ts_ms
                 FROM events
                 WHERE site_id = ?1 AND ts_ms >= ?2 AND ts_ms <= ?3",
            )
            .map_err(map_sql_err)?;
        let mut rows = stmt
            .query(params![site_blob, start_min, now_ms])
            .map_err(map_sql_err)?;
        while let Some(row) = rows.next().map_err(map_sql_err)? {
            let ts: i64 = row.get(0).map_err(map_sql_err)?;
            let minute_ms = ts.div_euclid(MS_PER_MINUTE) * MS_PER_MINUTE;
            *counts.entry(minute_ms).or_insert(0) += 1;
        }
    }

    let mut pulse = Vec::with_capacity(30);
    for idx in 0..PULSE_MINUTES {
        let minute_ms = start_min + idx * MS_PER_MINUTE;
        pulse.push(RealtimePulse {
            minute_ms,
            events: counts.get(&minute_ms).copied().unwrap_or(0),
        });
    }
    Ok(pulse)
}
