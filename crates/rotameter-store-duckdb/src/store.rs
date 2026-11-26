#![forbid(unsafe_code)]
//! `DuckDbStore` connection lifecycle, configuration, and capability metadata.

use std::fmt;
use std::path::Path;
use std::sync::Mutex;

use duckdb::Connection;
use rotameter_core::error::CoreError;
use rotameter_core::storage::{AnalyticsDataPlane, BoxFuture, StorageCapabilities};

use crate::schema::init_schema;

/// Runtime configuration options for [`DuckDbStore`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DuckDbConfig {
    /// Optional memory limit string passed to `DuckDB` `SET memory_limit = ...` (e.g. `"512MB"`).
    pub memory_limit: Option<String>,
    /// Optional worker thread cap passed to `DuckDB` `SET threads = ...`.
    pub threads: Option<u32>,
}

/// Bundled `DuckDB` OLAP backend implementing [`AnalyticsDataPlane`].
pub struct DuckDbStore {
    conn: Mutex<Connection>,
}

impl DuckDbStore {
    /// Opens or initializes a file-backed `DuckDB` analytics database at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if directories, `DuckDB` open, pragmas, or DDL init fails.
    pub fn open(path: impl AsRef<Path>, config: &DuckDbConfig) -> Result<Self, CoreError> {
        let db_path = path.as_ref();
        if let Some(parent) = db_path.parent()
            && !parent.as_os_str().is_empty()
        {
            std::fs::create_dir_all(parent).map_err(|err| {
                CoreError::Storage(format!("cannot create duckdb directory: {err}"))
            })?;
        }
        let conn = Connection::open(db_path).map_err(map_duck_err)?;
        configure_connection(&conn, config)?;
        init_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Opens an isolated in-memory `DuckDB` store for tests.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if `DuckDB` initialization fails.
    pub fn open_in_memory(config: &DuckDbConfig) -> Result<Self, CoreError> {
        let conn = Connection::open_in_memory().map_err(map_duck_err)?;
        configure_connection(&conn, config)?;
        init_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Returns static [`StorageCapabilities`] for `DuckDbStore` (OLAP-only).
    pub const fn capabilities_sync(&self) -> StorageCapabilities {
        StorageCapabilities {
            supports_raw_event_log: true,
            supports_arbitrary_distinct_visitors: true,
            supports_multi_node_concurrent_writes: false,
            supports_sql_funnels: true,
            supports_raw_append_only: true,
            supports_control_plane: false,
            supports_analytics_data_plane: true,
        }
    }

    /// Forces a `DuckDB` `CHECKPOINT` flush cleanly on graceful shutdown.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if checkpoint execution fails.
    pub fn checkpoint(&self) -> Result<(), CoreError> {
        self.with_conn(|conn| conn.execute_batch("CHECKPOINT").map_err(map_duck_err))
    }

    pub(crate) fn with_conn<F, T>(&self, f: F) -> Result<T, CoreError>
    where
        F: FnOnce(&Connection) -> Result<T, CoreError>,
    {
        let guard = self
            .conn
            .lock()
            .map_err(|err| CoreError::Storage(format!("duckdb mutex poisoned: {err}")))?;
        f(&guard)
    }
}

impl AnalyticsDataPlane for DuckDbStore {
    fn data_plane_capabilities(&self) -> StorageCapabilities {
        self.capabilities_sync()
    }

    fn flush_shutdown(&self) -> BoxFuture<'_, Result<(), CoreError>> {
        Box::pin(std::future::ready(self.checkpoint()))
    }
}

impl fmt::Debug for DuckDbStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DuckDbStore").finish_non_exhaustive()
    }
}

fn configure_connection(conn: &Connection, config: &DuckDbConfig) -> Result<(), CoreError> {
    if let Some(limit) = config.memory_limit.as_deref() {
        let escaped = limit.replace('\'', "");
        conn.execute_batch(&format!("SET memory_limit = '{escaped}';"))
            .map_err(map_duck_err)?;
    }
    if let Some(threads) = config.threads {
        conn.execute_batch(&format!("SET threads = {};", threads.max(1)))
            .map_err(map_duck_err)?;
    }
    Ok(())
}

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn map_duck_err(err: duckdb::Error) -> CoreError {
    CoreError::Storage(format!("duckdb error: {err}"))
}
