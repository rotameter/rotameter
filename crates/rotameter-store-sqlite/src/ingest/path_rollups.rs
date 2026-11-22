#![forbid(unsafe_code)]
//! Daily visitor deduplication and per-path rollup writers inside ingest transactions.

use std::collections::{HashMap, HashSet};

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::utc_yyyymmdd;
use rotameter_core::storage::{IngestEvent, SessionUpsert};
use rotameter_core::types::EventKind;
use rusqlite::{Transaction, params};

use crate::config::map_sql_err;

#[derive(Default)]
struct PathDayAcc {
    pageviews: i64,
    events_total: i64,
    sessions: HashSet<[u8; 16]>,
    visitors: HashSet<[u8; 16]>,
    bounced_visits: i64,
    total_duration_ms: i64,
}

pub(crate) fn record_daily_visitors(
    tx: &Transaction<'_>,
    sessions: &[SessionUpsert],
) -> Result<(), CoreError> {
    if sessions.is_empty() {
        return Ok(());
    }
    let mut seen_stmt = tx
        .prepare_cached(
            "INSERT OR IGNORE INTO seen_visitors_daily (site_id, day_utc, visitor_id)
             VALUES (?1, ?2, ?3)",
        )
        .map_err(map_sql_err)?;
    let mut day_deltas: HashMap<(SiteId, i64), i64> = HashMap::new();
    for sess in sessions {
        let day_utc = i64::from(utc_yyyymmdd(sess.started_at_ms));
        let inserted = seen_stmt
            .execute(params![
                sess.site_id.as_bytes().as_slice(),
                day_utc,
                sess.visitor_id.as_slice(),
            ])
            .map_err(map_sql_err)?;
        if inserted > 0 {
            *day_deltas.entry((sess.site_id, day_utc)).or_default() += 1;
        }
    }

    let mut rollup_stmt = tx
        .prepare_cached(
            "INSERT INTO rollups_daily (
               site_id, day_utc, pageviews, events_total,
               bounced_visits, visits_started, total_duration_ms, visitors
             ) VALUES (?1, ?2, 0, 0, 0, 0, 0, ?3)
             ON CONFLICT(site_id, day_utc) DO UPDATE SET
               visitors = visitors + excluded.visitors",
        )
        .map_err(map_sql_err)?;
    for ((site_id, day_utc), delta) in day_deltas {
        rollup_stmt
            .execute(params![site_id.as_bytes().as_slice(), day_utc, delta])
            .map_err(map_sql_err)?;
    }
    Ok(())
}

pub(crate) fn write_path_rollups(
    tx: &Transaction<'_>,
    events: &[IngestEvent],
    path_ids: &[i64],
    sessions: &[SessionUpsert],
) -> Result<(), CoreError> {
    if events.is_empty() {
        return Ok(());
    }
    let mut sess_meta: HashMap<(SiteId, [u8; 16]), (bool, i64)> =
        HashMap::with_capacity(sessions.len());
    for s in sessions {
        let is_bounced = s.event_count <= 1;
        let dur_ms = (s.last_event_ms - s.started_at_ms).max(0);
        sess_meta.insert((s.site_id, s.session_id), (is_bounced, dur_ms));
    }

    let mut agg: HashMap<(SiteId, i64, i64), PathDayAcc> = HashMap::new();
    for (ev, &path_id) in events.iter().zip(path_ids.iter()) {
        let day_utc = i64::from(utc_yyyymmdd(ev.ts_ms));
        let entry = agg.entry((ev.site_id, day_utc, path_id)).or_default();
        entry.events_total += 1;
        if ev.event_kind == EventKind::Pageview {
            entry.pageviews += 1;
        }
        entry.visitors.insert(ev.visitor_id);
        if entry.sessions.insert(ev.session_id)
            && let Some(&(is_bounced, dur_ms)) = sess_meta.get(&(ev.site_id, ev.session_id))
        {
            if is_bounced {
                entry.bounced_visits += 1;
            }
            entry.total_duration_ms += dur_ms;
        }
    }

    let mut stmt = tx
        .prepare_cached(
            "INSERT INTO rollups_path_daily (
               site_id, day_utc, path_id, pageviews, events_total,
               visits, visitors, bounced_visits, total_duration_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(site_id, day_utc, path_id) DO UPDATE SET
               pageviews = pageviews + excluded.pageviews,
               events_total = events_total + excluded.events_total,
               visits = visits + excluded.visits,
               visitors = visitors + excluded.visitors,
               bounced_visits = bounced_visits + excluded.bounced_visits,
               total_duration_ms = total_duration_ms + excluded.total_duration_ms",
        )
        .map_err(map_sql_err)?;
    for ((site_id, day_utc, path_id), row) in agg {
        let visits = i64::try_from(row.sessions.len()).unwrap_or(i64::MAX);
        let visitors = i64::try_from(row.visitors.len()).unwrap_or(i64::MAX);
        stmt.execute(params![
            site_id.as_bytes().as_slice(),
            day_utc,
            path_id,
            row.pageviews,
            row.events_total,
            visits,
            visitors,
            row.bounced_visits,
            row.total_duration_ms,
        ])
        .map_err(map_sql_err)?;
    }
    Ok(())
}
