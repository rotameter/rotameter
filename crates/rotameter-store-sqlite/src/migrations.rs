#![forbid(unsafe_code)]
//! Numbered forward `SQLite` schema migrations.

use rotameter_core::error::CoreError;
use rusqlite::{Connection, TransactionBehavior, params};

use crate::config::map_sql_err;

const MIGRATIONS: &[(u32, &str, &str)] = &[
    (
        1,
        "0001_initial",
        include_str!("../migrations/0001_initial.sql"),
    ),
    (
        2,
        "0002_query_rollups",
        include_str!("../migrations/0002_query_rollups.sql"),
    ),
    (
        3,
        "0003_split_dim_strings",
        include_str!("../migrations/0003_split_dim_strings.sql"),
    ),
    (
        4,
        "0004_salts",
        include_str!("../migrations/0004_salts.sql"),
    ),
];

/// Applies all pending forward migrations atomically and returns the latest schema version.
///
/// # Errors
///
/// Returns [`CoreError::Storage`] if a migration statement or transaction fails.
pub fn run_migrations(conn: &mut Connection, now_ms: i64) -> Result<u32, CoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
           version       INTEGER PRIMARY KEY,
           name          TEXT NOT NULL,
           applied_at_ms INTEGER NOT NULL
         );",
    )
    .map_err(map_sql_err)?;

    let mut applied = current_schema_version(conn)?;
    for &(version, name, sql) in MIGRATIONS {
        if version <= applied {
            continue;
        }
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(map_sql_err)?;
        tx.execute_batch(sql).map_err(map_sql_err)?;
        tx.execute(
            "INSERT INTO schema_migrations (version, name, applied_at_ms) VALUES (?1, ?2, ?3)",
            params![version, name, now_ms],
        )
        .map_err(map_sql_err)?;
        tx.commit().map_err(map_sql_err)?;
        applied = version;
    }

    Ok(applied)
}

/// Returns the highest applied migration version recorded in `schema_migrations`.
///
/// # Errors
///
/// Returns [`CoreError::Storage`] if querying `SQLite` fails.
pub fn current_schema_version(conn: &Connection) -> Result<u32, CoreError> {
    let table_exists: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations'",
            [],
            |row| row.get(0),
        )
        .map_err(map_sql_err)?;
    if table_exists == 0 {
        return Ok(0);
    }
    let version: u32 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .map_err(map_sql_err)?;
    Ok(version)
}
