#![forbid(unsafe_code)]
//! Typed `rusqlite::Row` decoding helpers for control-plane and session records.

use rotameter_core::id::{KeyId, OrgId, SiteId};
use rotameter_core::session::SessionSnapshot;
use rotameter_core::types::{ApiKeyRecord, KeyScope, Org, SaltRotation, Site};
use rusqlite::{Error as SqlError, Row, types::Type};

fn decode_blob<const N: usize>(bytes: &[u8], col: usize) -> rusqlite::Result<[u8; N]> {
    bytes.try_into().map_err(|_| {
        SqlError::FromSqlConversionFailure(
            col,
            Type::Blob,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("expected {N}-byte BLOB, got {} bytes", bytes.len()),
            )),
        )
    })
}

fn parse_json_vec(raw: &str, col: usize) -> rusqlite::Result<Vec<String>> {
    serde_json::from_str(raw)
        .map_err(|err| SqlError::FromSqlConversionFailure(col, Type::Text, Box::new(err)))
}

fn parse_salt_rotation(raw: &str, col: usize) -> rusqlite::Result<SaltRotation> {
    match raw {
        "daily" => Ok(SaltRotation::Daily),
        "weekly" => Ok(SaltRotation::Weekly),
        "monthly" => Ok(SaltRotation::Monthly),
        other => Err(SqlError::FromSqlConversionFailure(
            col,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown salt_rotation `{other}`"),
            )),
        )),
    }
}

fn parse_key_scope(raw: &str, col: usize) -> rusqlite::Result<KeyScope> {
    match raw {
        "ingest" => Ok(KeyScope::Ingest),
        "read" => Ok(KeyScope::Read),
        "admin" => Ok(KeyScope::Admin),
        other => Err(SqlError::FromSqlConversionFailure(
            col,
            Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown key scope `{other}`"),
            )),
        )),
    }
}

pub(crate) fn decode_org(row: &Row<'_>) -> rusqlite::Result<Org> {
    let raw_id: Vec<u8> = row.get(0)?;
    Ok(Org {
        org_id: OrgId::from_bytes(decode_blob::<16>(&raw_id, 0)?),
        slug: row.get(1)?,
        display_name: row.get(2)?,
        created_at_ms: row.get(3)?,
        updated_at_ms: row.get(4)?,
    })
}

pub(crate) fn decode_site(row: &Row<'_>) -> rusqlite::Result<Site> {
    let raw_site_id: Vec<u8> = row.get(0)?;
    let raw_org_id: Vec<u8> = row.get(1)?;
    let raw_rot: String = row.get(5)?;
    let respect_gpc: i64 = row.get(6)?;
    let is_self_site: i64 = row.get(7)?;
    let raw_origins: String = row.get(8)?;
    let raw_query_keys: String = row.get(9)?;
    let raw_retention: Option<i64> = row.get(10)?;

    Ok(Site {
        site_id: SiteId::from_bytes(decode_blob::<16>(&raw_site_id, 0)?),
        org_id: OrgId::from_bytes(decode_blob::<16>(&raw_org_id, 1)?),
        slug: row.get(2)?,
        display_name: row.get(3)?,
        timezone: row.get(4)?,
        salt_rotation: parse_salt_rotation(&raw_rot, 5)?,
        respect_gpc: respect_gpc != 0,
        is_self_site: is_self_site != 0,
        allowed_origins: parse_json_vec(&raw_origins, 8)?,
        allowed_query_keys: parse_json_vec(&raw_query_keys, 9)?,
        raw_retention_days: raw_retention.and_then(|d| u32::try_from(d).ok()),
        created_at_ms: row.get(11)?,
        updated_at_ms: row.get(12)?,
    })
}

pub(crate) fn decode_api_key(row: &Row<'_>) -> rusqlite::Result<ApiKeyRecord> {
    let raw_key_id: Vec<u8> = row.get(0)?;
    let raw_org_id: Vec<u8> = row.get(1)?;
    let raw_hash: Vec<u8> = row.get(3)?;
    let raw_scope: String = row.get(4)?;
    let raw_site_id: Option<Vec<u8>> = row.get(5)?;

    let site_id = match raw_site_id {
        Some(bytes) => Some(SiteId::from_bytes(decode_blob::<16>(&bytes, 5)?)),
        None => None,
    };

    Ok(ApiKeyRecord {
        key_id: KeyId::from_bytes(decode_blob::<16>(&raw_key_id, 0)?),
        org_id: OrgId::from_bytes(decode_blob::<16>(&raw_org_id, 1)?),
        key_prefix: row.get(2)?,
        key_hash: decode_blob::<32>(&raw_hash, 3)?,
        scope: parse_key_scope(&raw_scope, 4)?,
        site_id,
        label: row.get(6)?,
        created_at_ms: row.get(7)?,
        last_used_ms: row.get(8)?,
        revoked_at_ms: row.get(9)?,
    })
}

pub(crate) fn decode_session_snapshot(row: &Row<'_>) -> rusqlite::Result<SessionSnapshot> {
    let raw_session: Vec<u8> = row.get(0)?;
    let raw_visitor: Vec<u8> = row.get(1)?;
    let event_count: i64 = row.get(4)?;
    let pageview_count: i64 = row.get(5)?;
    let ec = u32::try_from(event_count.max(0)).unwrap_or(u32::MAX);
    let pvc = u32::try_from(pageview_count.max(0)).unwrap_or(u32::MAX);

    Ok(SessionSnapshot {
        session_id: decode_blob::<16>(&raw_session, 0)?,
        visitor_id: decode_blob::<16>(&raw_visitor, 1)?,
        started_at_ms: row.get(2)?,
        last_event_ms: row.get(3)?,
        event_count: ec,
        pageview_count: pvc,
        last_seq: ec,
    })
}
