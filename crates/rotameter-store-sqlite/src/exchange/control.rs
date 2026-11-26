#![forbid(unsafe_code)]
//! Control-plane export and idempotent import helpers for `SQLite` exchange.

use rotameter_core::error::CoreError;
use rotameter_core::storage::{ControlPlaneSnapshot, RotatingSaltRecord};
use rotameter_core::types::{KeyScope, SaltRotation};
use rusqlite::{Connection, TransactionBehavior, params};

use crate::config::map_sql_err;
use crate::row_decode::{decode_api_key, decode_org, decode_site};

pub(crate) fn export_control_plane_rows(
    conn: &Connection,
) -> Result<ControlPlaneSnapshot, CoreError> {
    let mut org_stmt = conn
        .prepare(
            "SELECT org_id, slug, display_name, created_at_ms, updated_at_ms
             FROM orgs ORDER BY created_at_ms ASC, slug ASC",
        )
        .map_err(map_sql_err)?;
    let orgs = org_stmt
        .query_map([], decode_org)
        .map_err(map_sql_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sql_err)?;

    let mut site_stmt = conn
        .prepare(
            "SELECT site_id, org_id, slug, display_name, timezone, salt_rotation,
                    respect_gpc, is_self_site, allowed_origins, allowed_query_keys,
                    raw_retention_days, created_at_ms, updated_at_ms
             FROM sites ORDER BY created_at_ms ASC, slug ASC",
        )
        .map_err(map_sql_err)?;
    let sites = site_stmt
        .query_map([], decode_site)
        .map_err(map_sql_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sql_err)?;

    let mut key_stmt = conn
        .prepare(
            "SELECT key_id, org_id, key_prefix, key_hash, scope, site_id, label,
                    created_at_ms, last_used_ms, revoked_at_ms
             FROM api_keys ORDER BY created_at_ms ASC",
        )
        .map_err(map_sql_err)?;
    let api_keys = key_stmt
        .query_map([], decode_api_key)
        .map_err(map_sql_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sql_err)?;

    let mut salt_stmt = conn
        .prepare(
            "SELECT rotation, window_id, salt, created_at_ms
             FROM rotating_salts ORDER BY rotation ASC, window_id ASC",
        )
        .map_err(map_sql_err)?;
    let rotating_salts = salt_stmt
        .query_map([], decode_rotating_salt)
        .map_err(map_sql_err)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(map_sql_err)?;

    Ok(ControlPlaneSnapshot {
        orgs,
        sites,
        api_keys,
        rotating_salts,
    })
}

pub(crate) fn import_control_plane_rows(
    conn: &mut Connection,
    snap: &ControlPlaneSnapshot,
) -> Result<(), CoreError> {
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(map_sql_err)?;

    for org in &snap.orgs {
        tx.execute(
            "INSERT OR REPLACE INTO orgs (org_id, slug, display_name, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                org.org_id.as_bytes().as_slice(),
                org.slug,
                org.display_name,
                org.created_at_ms,
                org.updated_at_ms,
            ],
        )
        .map_err(map_sql_err)?;
    }

    for site in &snap.sites {
        let origins_json = serde_json::to_string(&site.allowed_origins)
            .map_err(|err| CoreError::Storage(err.to_string()))?;
        let keys_json = serde_json::to_string(&site.allowed_query_keys)
            .map_err(|err| CoreError::Storage(err.to_string()))?;
        tx.execute(
            "INSERT OR REPLACE INTO sites (
               site_id, org_id, slug, display_name, timezone, salt_rotation,
               respect_gpc, is_self_site, allowed_origins, allowed_query_keys,
               raw_retention_days, created_at_ms, updated_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
            params![
                site.site_id.as_bytes().as_slice(),
                site.org_id.as_bytes().as_slice(),
                site.slug,
                site.display_name,
                site.timezone,
                site.salt_rotation.as_str(),
                i64::from(site.respect_gpc),
                i64::from(site.is_self_site),
                origins_json,
                keys_json,
                site.raw_retention_days.map(i64::from),
                site.created_at_ms,
                site.updated_at_ms,
            ],
        )
        .map_err(map_sql_err)?;
    }

    for key in &snap.api_keys {
        let scope = match key.scope {
            KeyScope::Ingest => "ingest",
            KeyScope::Read => "read",
            KeyScope::Admin => "admin",
        };
        let site_blob = key.site_id.as_ref().map(|id| id.as_bytes().as_slice());
        tx.execute(
            "INSERT OR IGNORE INTO api_keys (
               key_id, org_id, key_prefix, key_hash, scope, site_id, label,
               created_at_ms, last_used_ms, revoked_at_ms
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                key.key_id.as_bytes().as_slice(),
                key.org_id.as_bytes().as_slice(),
                key.key_prefix,
                key.key_hash.as_slice(),
                scope,
                site_blob,
                key.label,
                key.created_at_ms,
                key.last_used_ms,
                key.revoked_at_ms,
            ],
        )
        .map_err(map_sql_err)?;
    }

    for salt in &snap.rotating_salts {
        tx.execute(
            "INSERT OR IGNORE INTO rotating_salts (rotation, window_id, salt, created_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                salt.rotation.as_str(),
                salt.window_id,
                salt.salt.as_slice(),
                salt.created_at_ms,
            ],
        )
        .map_err(map_sql_err)?;
    }

    tx.commit().map_err(map_sql_err)
}

fn decode_rotating_salt(row: &rusqlite::Row<'_>) -> rusqlite::Result<RotatingSaltRecord> {
    let rot_str: String = row.get(0)?;
    let rotation = SaltRotation::parse(&rot_str).ok_or_else(|| {
        rusqlite::Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("unknown salt rotation `{rot_str}`"),
            )),
        )
    })?;
    let blob: Vec<u8> = row.get(2)?;
    let salt: [u8; 32] = blob.try_into().map_err(|_| {
        rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Blob,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "rotating salt must be 32 bytes",
            )),
        )
    })?;
    Ok(RotatingSaltRecord {
        rotation,
        window_id: row.get(1)?,
        salt,
        created_at_ms: row.get(3)?,
    })
}
