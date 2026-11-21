#![forbid(unsafe_code)]
//! Online `SQLite` `VACUUM INTO` snapshot backups, WAL checkpoints, and integrity checks.

use std::path::Path;

use rotameter_core::error::CoreError;
use rusqlite::Connection;

use crate::config::map_sql_err;

/// Writes a consistent point-in-time `SQLite` backup to `dest` via `VACUUM INTO`.
///
/// Refuses to overwrite an existing destination file.
///
/// # Errors
///
/// Returns [`CoreError::Storage`] if `dest` already exists, is non-UTF-8, or `VACUUM INTO` fails.
pub fn vacuum_into(conn: &Connection, dest: &Path) -> Result<(), CoreError> {
    if dest.exists() {
        return Err(CoreError::Storage(format!(
            "backup destination already exists: {}",
            dest.display()
        )));
    }
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent)
            .map_err(|err| CoreError::Storage(format!("cannot create backup dir: {err}")))?;
    }
    let dest_str = dest
        .to_str()
        .ok_or_else(|| CoreError::Storage("backup path is not valid UTF-8".to_string()))?;
    conn.execute("VACUUM INTO ?1", [dest_str])
        .map_err(map_sql_err)?;
    Ok(())
}

/// Runs `PRAGMA integrity_check` and returns `true` when `SQLite` reports `"ok"`.
///
/// # Errors
///
/// Returns [`CoreError::Storage`] if executing `PRAGMA integrity_check` fails.
pub fn verify_integrity(conn: &Connection) -> Result<bool, CoreError> {
    let result: String = conn
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .map_err(map_sql_err)?;
    Ok(result.eq_ignore_ascii_case("ok"))
}

/// Checkpoints and truncates the WAL journal (`PRAGMA wal_checkpoint(TRUNCATE)`).
///
/// # Errors
///
/// Returns [`CoreError::Storage`] if checkpointing fails.
pub fn checkpoint_truncate(conn: &Connection) -> Result<(), CoreError> {
    conn.execute_batch("PRAGMA wal_checkpoint(TRUNCATE);")
        .map_err(map_sql_err)
}
