#![forbid(unsafe_code)]
//! `SQLite` persistence helpers for hashed API keys (`api_keys` table).

use rotameter_core::error::CoreError;
use rotameter_core::id::{KeyId, OrgId};
use rotameter_core::types::{ApiKeyRecord, KeyScope};
use rusqlite::{Connection, OptionalExtension, params};

use crate::config::map_sql_err;
use crate::row_decode::decode_api_key;

const KEY_SELECT: &str = "SELECT key_id, org_id, key_prefix, key_hash, scope, site_id, label,
                                 created_at_ms, last_used_ms, revoked_at_ms
                          FROM api_keys";

pub(crate) fn insert_api_key(conn: &Connection, record: &ApiKeyRecord) -> Result<(), CoreError> {
    let scope_str = match record.scope {
        KeyScope::Ingest => "ingest",
        KeyScope::Read => "read",
        KeyScope::Admin => "admin",
    };
    let site_blob = record.site_id.as_ref().map(|id| id.as_bytes().as_slice());
    conn.execute(
        "INSERT INTO api_keys (
           key_id, org_id, key_prefix, key_hash, scope, site_id, label,
           created_at_ms, last_used_ms, revoked_at_ms
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            record.key_id.as_bytes().as_slice(),
            record.org_id.as_bytes().as_slice(),
            record.key_prefix,
            record.key_hash.as_slice(),
            scope_str,
            site_blob,
            record.label,
            record.created_at_ms,
            record.last_used_ms,
            record.revoked_at_ms,
        ],
    )
    .map_err(map_sql_err)?;
    Ok(())
}

pub(crate) fn query_active_key_by_hash(
    conn: &Connection,
    key_hash: &[u8; 32],
) -> Result<Option<ApiKeyRecord>, CoreError> {
    let sql = format!("{KEY_SELECT} WHERE key_hash = ?1 AND revoked_at_ms IS NULL");
    conn.query_row(&sql, [key_hash.as_slice()], decode_api_key)
        .optional()
        .map_err(map_sql_err)
}

pub(crate) fn query_api_keys(
    conn: &Connection,
    org_id: OrgId,
) -> Result<Vec<ApiKeyRecord>, CoreError> {
    let sql = format!("{KEY_SELECT} WHERE org_id = ?1 ORDER BY created_at_ms ASC");
    let mut stmt = conn.prepare(&sql).map_err(map_sql_err)?;
    let rows = stmt
        .query_map([org_id.as_bytes().as_slice()], decode_api_key)
        .map_err(map_sql_err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(map_sql_err)
}

pub(crate) fn mark_key_revoked(
    conn: &Connection,
    key_id: KeyId,
    revoked_at_ms: i64,
) -> Result<bool, CoreError> {
    let updated = conn
        .execute(
            "UPDATE api_keys SET revoked_at_ms = ?2 WHERE key_id = ?1 AND revoked_at_ms IS NULL",
            params![key_id.as_bytes().as_slice(), revoked_at_ms],
        )
        .map_err(map_sql_err)?;
    Ok(updated > 0)
}
