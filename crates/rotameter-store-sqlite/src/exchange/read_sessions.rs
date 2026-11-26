#![forbid(unsafe_code)]
//! Chunked `sessions` reader joining interned string dimensions (`SQLite` exchange).

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::storage::SessionUpsert;
use rotameter_core::types::{Channel, DeviceType, UtmParams, ViewportClass};
use rusqlite::{Connection, Row, params};

use crate::config::map_sql_err;
use crate::exchange::read::{blob_col, i64_limit, invalid_col};

pub(crate) fn read_sessions_chunk(
    conn: &Connection,
    offset: usize,
    limit: usize,
) -> Result<Vec<SessionUpsert>, CoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT s.site_id, s.session_id, s.visitor_id, s.started_at_ms, s.last_event_ms,
                    s.event_count, s.pageview_count,
                    dep.value, dxp.value, drh.value, dch.value,
                    dus.value, dum.value, duc.value,
                    dco.value, dreg.value, dbr.value, dos.value,
                    s.device_type, s.viewport_class
             FROM sessions s
             JOIN dim_strings dep  ON dep.id  = s.entry_path_id
             JOIN dim_strings dxp  ON dxp.id  = s.exit_path_id
             JOIN dim_strings drh  ON drh.id  = s.ref_host_id
             JOIN dim_strings dch  ON dch.id  = s.channel_id
             JOIN dim_strings dus  ON dus.id  = s.utm_source_id
             JOIN dim_strings dum  ON dum.id  = s.utm_medium_id
             JOIN dim_strings duc  ON duc.id  = s.utm_campaign_id
             JOIN dim_strings dco  ON dco.id  = s.country_id
             JOIN dim_strings dreg ON dreg.id = s.region_id
             JOIN dim_strings dbr  ON dbr.id  = s.browser_id
             JOIN dim_strings dos  ON dos.id  = s.os_id
             ORDER BY s.site_id, s.session_id
             LIMIT ?1 OFFSET ?2",
        )
        .map_err(map_sql_err)?;
    stmt.query_map(params![i64_limit(limit), i64_limit(offset)], decode_session)
        .map_err(map_sql_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sql_err)
}

fn decode_session(row: &Row<'_>) -> rusqlite::Result<SessionUpsert> {
    let ec_raw: i64 = row.get(5)?;
    let pvc_raw: i64 = row.get(6)?;
    let ch_raw: String = row.get(10)?;
    let channel = Channel::parse(&ch_raw).ok_or_else(|| {
        invalid_col(
            10,
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown channel `{ch_raw}`"),
            ),
        )
    })?;
    let dev_raw: i64 = row.get(18)?;
    let vp_raw: i64 = row.get(19)?;

    Ok(SessionUpsert {
        site_id: SiteId::from_bytes(blob_col::<16>(row, 0)?),
        session_id: blob_col::<16>(row, 1)?,
        visitor_id: blob_col::<16>(row, 2)?,
        started_at_ms: row.get(3)?,
        last_event_ms: row.get(4)?,
        event_count: u32::try_from(ec_raw.max(0)).map_err(|err| invalid_col(5, err))?,
        pageview_count: u32::try_from(pvc_raw.max(0)).map_err(|err| invalid_col(6, err))?,
        entry_path: row.get(7)?,
        exit_path: row.get(8)?,
        ref_host: row.get(9)?,
        channel,
        utm: UtmParams {
            source: non_empty(row.get(11)?),
            medium: non_empty(row.get(12)?),
            campaign: non_empty(row.get(13)?),
            content: None,
            term: None,
        },
        country: row.get(14)?,
        region: row.get(15)?,
        browser: row.get(16)?,
        os: row.get(17)?,
        device_type: DeviceType::from_u8(u8::try_from(dev_raw.max(0)).unwrap_or(0)),
        viewport_class: ViewportClass::from_u8(u8::try_from(vp_raw.max(0)).unwrap_or(0)),
    })
}

fn non_empty(val: String) -> Option<String> {
    (!val.is_empty()).then_some(val)
}
