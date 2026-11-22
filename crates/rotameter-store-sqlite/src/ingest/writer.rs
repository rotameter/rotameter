#![forbid(unsafe_code)]
//! Batched single-transaction event, session, and rollup write helpers.

use rotameter_core::error::CoreError;
use rotameter_core::storage::{IngestEvent, RollupDelta, SessionUpsert};
use rotameter_core::types::DimensionKind;
use rusqlite::{Transaction, params};

use crate::config::map_sql_err;
use crate::intern::DimInterner;

pub(crate) fn write_event(
    tx: &Transaction<'_>,
    interner: &mut DimInterner,
    ev: &IngestEvent,
) -> Result<i64, CoreError> {
    let event_name_id = interner.intern(
        tx,
        ev.site_id,
        DimensionKind::EventName,
        ev.event_kind.as_str(),
    )?;
    let path_id = interner.intern(tx, ev.site_id, DimensionKind::Path, &ev.path)?;
    let props_json = if ev.props.is_empty() {
        None
    } else {
        Some(
            serde_json::to_string(ev.props.as_map())
                .map_err(|err| CoreError::Storage(err.to_string()))?,
        )
    };

    let mut stmt = tx
        .prepare_cached(
            "INSERT OR IGNORE INTO events (
               site_id, ts_ms, session_id, seq_in_session, visitor_id,
               event_name_id, path_id, props_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .map_err(map_sql_err)?;
    stmt.execute(params![
        ev.site_id.as_bytes().as_slice(),
        ev.ts_ms,
        ev.session_id.as_slice(),
        i64::from(ev.seq_in_session),
        ev.visitor_id.as_slice(),
        event_name_id,
        path_id,
        props_json,
    ])
    .map_err(map_sql_err)?;
    Ok(path_id)
}

pub(crate) fn write_session(
    tx: &Transaction<'_>,
    interner: &mut DimInterner,
    s: &SessionUpsert,
) -> Result<(), CoreError> {
    let entry_path_id = interner.intern(tx, s.site_id, DimensionKind::Path, &s.entry_path)?;
    let exit_path_id = interner.intern(tx, s.site_id, DimensionKind::Path, &s.exit_path)?;
    let ref_host_id = interner.intern(tx, s.site_id, DimensionKind::RefHost, &s.ref_host)?;
    let channel_id = interner.intern(tx, s.site_id, DimensionKind::Channel, s.channel.as_str())?;
    let utm_source_id = interner.intern(
        tx,
        s.site_id,
        DimensionKind::Utm,
        s.utm.source.as_deref().unwrap_or(""),
    )?;
    let utm_medium_id = interner.intern(
        tx,
        s.site_id,
        DimensionKind::Utm,
        s.utm.medium.as_deref().unwrap_or(""),
    )?;
    let utm_campaign_id = interner.intern(
        tx,
        s.site_id,
        DimensionKind::Utm,
        s.utm.campaign.as_deref().unwrap_or(""),
    )?;
    let country_id = interner.intern(tx, s.site_id, DimensionKind::GeoCountry, &s.country)?;
    let region_id = interner.intern(tx, s.site_id, DimensionKind::GeoRegion, &s.region)?;
    let browser_id = interner.intern(tx, s.site_id, DimensionKind::UaBrowser, &s.browser)?;
    let os_id = interner.intern(tx, s.site_id, DimensionKind::UaOs, &s.os)?;

    let mut stmt = tx
        .prepare_cached(
            "INSERT INTO sessions (
               site_id, session_id, visitor_id, started_at_ms, last_event_ms,
               event_count, pageview_count, entry_path_id, exit_path_id, ref_host_id,
               channel_id, utm_source_id, utm_medium_id, utm_campaign_id, country_id,
               region_id, browser_id, os_id, device_type, viewport_class
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)
             ON CONFLICT(site_id, session_id) DO UPDATE SET
               last_event_ms = MAX(last_event_ms, excluded.last_event_ms),
               event_count = MAX(event_count, excluded.event_count),
               pageview_count = MAX(pageview_count, excluded.pageview_count),
               exit_path_id = excluded.exit_path_id",
        )
        .map_err(map_sql_err)?;
    stmt.execute(params![
        s.site_id.as_bytes().as_slice(),
        s.session_id.as_slice(),
        s.visitor_id.as_slice(),
        s.started_at_ms,
        s.last_event_ms,
        i64::from(s.event_count),
        i64::from(s.pageview_count),
        entry_path_id,
        exit_path_id,
        ref_host_id,
        channel_id,
        utm_source_id,
        utm_medium_id,
        utm_campaign_id,
        country_id,
        region_id,
        browser_id,
        os_id,
        i64::from(s.device_type.as_u8()),
        i64::from(s.viewport_class.as_u8()),
    ])
    .map_err(map_sql_err)?;
    Ok(())
}

pub(crate) fn write_rollup(tx: &Transaction<'_>, r: &RollupDelta) -> Result<(), CoreError> {
    let site_blob = r.site_id.as_bytes().as_slice();
    tx.prepare_cached(
        "INSERT OR IGNORE INTO rollups_hourly (
           site_id, bucket_ts_ms, pageviews, events_total,
           bounced_visits, visits_started, total_duration_ms
         ) VALUES (?1, ?2, 0, 0, 0, 0, 0)",
    )
    .map_err(map_sql_err)?
    .execute(params![site_blob, r.hour_bucket_ms])
    .map_err(map_sql_err)?;
    tx.prepare_cached(
        "UPDATE rollups_hourly SET
           pageviews = pageviews + ?3,
           events_total = events_total + ?4,
           bounced_visits = MAX(0, bounced_visits + ?5),
           visits_started = visits_started + ?6,
           total_duration_ms = total_duration_ms + ?7
         WHERE site_id = ?1 AND bucket_ts_ms = ?2",
    )
    .map_err(map_sql_err)?
    .execute(params![
        site_blob,
        r.hour_bucket_ms,
        r.pageviews,
        r.events_total,
        r.bounced_visits,
        r.visits_started,
        r.total_duration_ms,
    ])
    .map_err(map_sql_err)?;

    let day_utc = i64::from(r.day_utc);
    tx.prepare_cached(
        "INSERT OR IGNORE INTO rollups_daily (
           site_id, day_utc, pageviews, events_total,
           bounced_visits, visits_started, total_duration_ms
         ) VALUES (?1, ?2, 0, 0, 0, 0, 0)",
    )
    .map_err(map_sql_err)?
    .execute(params![site_blob, day_utc])
    .map_err(map_sql_err)?;
    tx.prepare_cached(
        "UPDATE rollups_daily SET
           pageviews = pageviews + ?3,
           events_total = events_total + ?4,
           bounced_visits = MAX(0, bounced_visits + ?5),
           visits_started = visits_started + ?6,
           total_duration_ms = total_duration_ms + ?7
         WHERE site_id = ?1 AND day_utc = ?2",
    )
    .map_err(map_sql_err)?
    .execute(params![
        site_blob,
        day_utc,
        r.pageviews,
        r.events_total,
        r.bounced_visits,
        r.visits_started,
        r.total_duration_ms,
    ])
    .map_err(map_sql_err)?;
    Ok(())
}
