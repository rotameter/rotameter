#![forbid(unsafe_code)]
//! `SQLite` persistence helpers for rotating CSPRNG privacy salts (`rotating_salts` table).

use rotameter_core::error::CoreError;
use rotameter_core::privacy::{SaltBytes, SaltWindow};
use rotameter_core::types::SaltRotation;
use rusqlite::{Connection, params};

use crate::config::map_sql_err;

pub(crate) fn get_or_create_salt_row(
    conn: &Connection,
    window: SaltWindow,
    candidate: SaltBytes,
    now_ms: i64,
) -> Result<SaltBytes, CoreError> {
    let rotation = window.rotation.as_str();
    conn.execute_batch("BEGIN IMMEDIATE").map_err(map_sql_err)?;
    let res = (|| {
        conn.execute(
            "INSERT OR IGNORE INTO rotating_salts (rotation, window_id, salt, created_at_ms)
             VALUES (?1, ?2, ?3, ?4)",
            params![rotation, window.window_id, candidate.as_slice(), now_ms],
        )
        .map_err(map_sql_err)?;

        let blob: Vec<u8> = conn
            .query_row(
                "SELECT salt FROM rotating_salts WHERE rotation = ?1 AND window_id = ?2",
                params![rotation, window.window_id],
                |row| row.get(0),
            )
            .map_err(map_sql_err)?;

        SaltBytes::try_from(blob.as_slice()).map_err(|_| {
            CoreError::Storage("corrupt rotating_salts row: salt length must be 32 bytes".into())
        })
    })();

    match res {
        Ok(salt) => {
            conn.execute_batch("COMMIT").map_err(map_sql_err)?;
            Ok(salt)
        }
        Err(err) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(err)
        }
    }
}

pub(crate) fn prune_salts_before_rows(
    conn: &Connection,
    rotation: SaltRotation,
    min_window_id: i64,
) -> Result<u64, CoreError> {
    let deleted = conn
        .execute(
            "DELETE FROM rotating_salts WHERE rotation = ?1 AND window_id < ?2",
            params![rotation.as_str(), min_window_id],
        )
        .map_err(map_sql_err)?;
    u64::try_from(deleted).map_err(|_| CoreError::Storage("deleted row count overflow".into()))
}
