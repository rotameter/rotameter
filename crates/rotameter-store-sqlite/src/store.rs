#![forbid(unsafe_code)]
//! `SqliteStore` lifecycle, reader/writer connection pools, backups, and health checks.

use std::fmt;
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use rotameter_core::error::CoreError;
use rusqlite::{Connection, OpenFlags};

use crate::backup::{checkpoint_truncate, vacuum_into, verify_integrity};
use crate::config::{
    SqliteConfig, SqliteHealthSnapshot, apply_reader_pragmas, apply_writer_pragmas, map_sql_err,
};
use crate::intern::DimInterner;
use crate::migrations::{current_schema_version, run_migrations};

pub(crate) struct WriterState {
    pub(crate) conn: Connection,
    pub(crate) interner: DimInterner,
}

/// Bundled `SQLite` WAL persistence backend implementing `rotameter-core` storage traits.
pub struct SqliteStore {
    writer: Mutex<WriterState>,
    readers: Vec<Mutex<Connection>>,
    reader_rr: AtomicUsize,
}

impl SqliteStore {
    /// Opens or initializes a file-backed `SQLite` WAL store and reader pool at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if directories, connections, PRAGMAs, or migrations fail.
    pub fn open(
        path: impl AsRef<Path>,
        config: SqliteConfig,
        now_ms: i64,
    ) -> Result<Self, CoreError> {
        let db_path = path.as_ref();
        if let Some(parent) = db_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent)
                .map_err(|err| CoreError::Storage(format!("cannot create db directory: {err}")))?;
        }

        let mut writer_conn = Connection::open(db_path).map_err(map_sql_err)?;
        apply_writer_pragmas(&writer_conn, config, true)?;
        run_migrations(&mut writer_conn, now_ms)?;

        let reader_count = config.reader_pool_size.max(1);
        let mut readers = Vec::with_capacity(reader_count);
        let read_flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
        for _ in 0..reader_count {
            let reader = Connection::open_with_flags(db_path, read_flags).map_err(map_sql_err)?;
            apply_reader_pragmas(&reader, config)?;
            readers.push(Mutex::new(reader));
        }

        Ok(Self {
            writer: Mutex::new(WriterState {
                conn: writer_conn,
                interner: DimInterner::new(config.intern_cache_capacity),
            }),
            readers,
            reader_rr: AtomicUsize::new(0),
        })
    }

    /// Opens an isolated in-memory `SQLite` store for fast unit and integration tests.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if opening, PRAGMA setup, or migrations fail.
    pub fn open_in_memory(config: SqliteConfig, now_ms: i64) -> Result<Self, CoreError> {
        let mut writer_conn = Connection::open_in_memory().map_err(map_sql_err)?;
        apply_writer_pragmas(&writer_conn, config, false)?;
        run_migrations(&mut writer_conn, now_ms)?;

        Ok(Self {
            writer: Mutex::new(WriterState {
                conn: writer_conn,
                interner: DimInterner::new(config.intern_cache_capacity),
            }),
            readers: Vec::new(),
            reader_rr: AtomicUsize::new(0),
        })
    }

    /// Returns the highest applied migration version.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if the reader lock or query fails.
    pub fn schema_version(&self) -> Result<u32, CoreError> {
        self.with_reader(current_schema_version)
    }

    /// Reads active PRAGMA settings, schema version, and `PRAGMA integrity_check` status.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if inspecting the store fails.
    pub fn health_snapshot(&self) -> Result<SqliteHealthSnapshot, CoreError> {
        self.with_reader(|conn| {
            let journal_mode: String = conn
                .query_row("PRAGMA journal_mode", [], |row| row.get(0))
                .map_err(map_sql_err)?;
            let cache_size: i64 = conn
                .query_row("PRAGMA cache_size", [], |row| row.get(0))
                .map_err(map_sql_err)?;
            let mmap_size: i64 = conn
                .query_row("PRAGMA mmap_size", [], |row| row.get(0))
                .map_err(map_sql_err)?;
            let schema_version = current_schema_version(conn)?;
            let integrity_ok = verify_integrity(conn)?;
            Ok(SqliteHealthSnapshot {
                journal_mode,
                schema_version,
                cache_size,
                mmap_size,
                integrity_ok,
            })
        })
    }

    /// Checkpoints and truncates the WAL journal cleanly (used during graceful drain).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if checkpointing fails.
    pub fn checkpoint_truncate(&self) -> Result<(), CoreError> {
        self.with_writer(|state| checkpoint_truncate(&state.conn))
    }

    /// Executes an online `VACUUM INTO` snapshot backup without blocking concurrent readers.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if `dest` already exists or `VACUUM INTO` fails.
    pub fn vacuum_into(&self, dest: impl AsRef<Path>) -> Result<(), CoreError> {
        let path = dest.as_ref();
        self.with_writer(|state| vacuum_into(&state.conn, path))
    }

    pub(crate) fn with_writer<F, T>(&self, f: F) -> Result<T, CoreError>
    where
        F: FnOnce(&mut WriterState) -> Result<T, CoreError>,
    {
        let mut guard = self
            .writer
            .lock()
            .map_err(|err| CoreError::Storage(format!("writer mutex poisoned: {err}")))?;
        f(&mut guard)
    }

    pub(crate) fn with_reader<F, T>(&self, f: F) -> Result<T, CoreError>
    where
        F: FnOnce(&Connection) -> Result<T, CoreError>,
    {
        if self.readers.is_empty() {
            return self.with_writer(|state| f(&state.conn));
        }
        let idx = self.reader_rr.fetch_add(1, Ordering::Relaxed) % self.readers.len();
        let reader_mutex = self
            .readers
            .get(idx)
            .ok_or_else(|| CoreError::Storage("reader pool is empty".to_string()))?;
        let guard = reader_mutex
            .lock()
            .map_err(|err| CoreError::Storage(format!("reader mutex poisoned: {err}")))?;
        f(&guard)
    }
}

impl fmt::Debug for SqliteStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SqliteStore")
            .field("reader_count", &self.readers.len())
            .finish_non_exhaustive()
    }
}
