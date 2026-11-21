#![forbid(unsafe_code)]
//! `SQLite` WAL connection configuration, PRAGMA tuning, and health diagnostics.

use rotameter_core::error::CoreError;
use rusqlite::Connection;

/// Resource-bounded configuration for [`crate::SqliteStore`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SqliteConfig {
    /// Number of read-only pooled connections opened for file-backed stores (`2..=4`).
    pub reader_pool_size: usize,
    /// `PRAGMA busy_timeout` in milliseconds (`5_000`).
    pub busy_timeout_ms: u64,
    /// `PRAGMA cache_size` (`-16_000` KiB = 16 MB page cache).
    pub cache_size_kib: i64,
    /// `PRAGMA mmap_size` ceiling in bytes (`67_108_864` = 64 MB).
    pub mmap_size_bytes: u64,
    /// Maximum cached `(site_id, kind, value) -> id` entries in the writer interner (`50_000`).
    pub intern_cache_capacity: usize,
}

impl Default for SqliteConfig {
    fn default() -> Self {
        Self {
            reader_pool_size: 3,
            busy_timeout_ms: 5_000,
            cache_size_kib: -16_000,
            mmap_size_bytes: 67_108_864,
            intern_cache_capacity: 50_000,
        }
    }
}

/// Live PRAGMA and schema diagnostics reported by [`crate::SqliteStore::health_snapshot`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SqliteHealthSnapshot {
    /// Active `PRAGMA journal_mode` value (e.g., `"wal"`).
    pub journal_mode: String,
    /// Latest applied schema migration number.
    pub schema_version: u32,
    /// Configured `PRAGMA cache_size` value.
    pub cache_size: i64,
    /// Configured `PRAGMA mmap_size` value.
    pub mmap_size: i64,
    /// Result of `PRAGMA integrity_check`.
    pub integrity_ok: bool,
}

/// Applies canonical WAL and memory-ceiling PRAGMAs to the singleton writer connection.
///
/// # Errors
///
/// Returns [`CoreError::Storage`] if `SQLite` rejects any PRAGMA.
pub fn apply_writer_pragmas(
    conn: &Connection,
    cfg: SqliteConfig,
    is_file_backed: bool,
) -> Result<(), CoreError> {
    let journal = if is_file_backed {
        "PRAGMA journal_mode = WAL;"
    } else {
        ""
    };
    let sql = format!(
        "{journal}\
         PRAGMA synchronous = NORMAL;\
         PRAGMA foreign_keys = ON;\
         PRAGMA busy_timeout = {busy};\
         PRAGMA cache_size = {cache};\
         PRAGMA mmap_size = {mmap};\
         PRAGMA temp_store = MEMORY;",
        busy = cfg.busy_timeout_ms,
        cache = cfg.cache_size_kib,
        mmap = cfg.mmap_size_bytes,
    );
    conn.execute_batch(&sql).map_err(map_sql_err)
}

/// Applies read-only PRAGMAs to a pooled reader connection.
///
/// # Errors
///
/// Returns [`CoreError::Storage`] if `SQLite` rejects any PRAGMA.
pub fn apply_reader_pragmas(conn: &Connection, cfg: SqliteConfig) -> Result<(), CoreError> {
    let sql = format!(
        "PRAGMA query_only = ON;\
         PRAGMA foreign_keys = ON;\
         PRAGMA busy_timeout = {busy};\
         PRAGMA cache_size = {cache};\
         PRAGMA mmap_size = {mmap};\
         PRAGMA temp_store = MEMORY;",
        busy = cfg.busy_timeout_ms,
        cache = cfg.cache_size_kib,
        mmap = cfg.mmap_size_bytes,
    );
    conn.execute_batch(&sql).map_err(map_sql_err)
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn map_sql_err(err: rusqlite::Error) -> CoreError {
    CoreError::Storage(err.to_string())
}
