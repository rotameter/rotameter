#![forbid(unsafe_code)]
//! Async `tokio::task::spawn_blocking` adapter wrapping [`DuckDbStore`] for `rotameter serve`.

use std::path::Path;
use std::sync::Arc;

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::VisitorId;
use rotameter_core::query::QuerySpec;
use rotameter_core::session::SessionSnapshot;
use rotameter_core::storage::{
    AnalyticsDataPlane, AnalyticsQueryStore, BoxFuture, EventIngestStore, QueryResult,
    RealtimeSnapshot, StorageCapabilities, WriteBatch,
};
use rotameter_core::types::Site;
use rotameter_store_duckdb::{DuckDbConfig, DuckDbStore};

/// Thread-safe async wrapper around [`DuckDbStore`] running `DuckDB` I/O on Tokio's blocking pool.
#[derive(Clone)]
pub(crate) struct DuckDbServerStore(Arc<DuckDbStore>);

impl DuckDbServerStore {
    /// Opens a file-backed `DuckDB` analytics database at `path` for server mode.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if initialization fails.
    pub(crate) fn open(path: impl AsRef<Path>, config: &DuckDbConfig) -> Result<Self, CoreError> {
        Ok(Self(Arc::new(DuckDbStore::open(path, config)?)))
    }

    pub(crate) fn inner(&self) -> Arc<DuckDbStore> {
        Arc::clone(&self.0)
    }
}

impl EventIngestStore for DuckDbServerStore {
    fn lookup_active_session(
        &self,
        site_id: SiteId,
        visitor_id: VisitorId,
        min_last_event_ms: i64,
    ) -> BoxFuture<'_, Result<Option<SessionSnapshot>, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || {
            s.lookup_active_session_sync(site_id, visitor_id, min_last_event_ms)
        }))
    }

    fn write_batch<'a>(&'a self, batch: &'a WriteBatch) -> BoxFuture<'a, Result<(), CoreError>> {
        let s = Arc::clone(&self.0);
        let batch = batch.clone();
        Box::pin(run_blocking(move || s.write_batch_sync(&batch)))
    }
}

impl AnalyticsQueryStore for DuckDbServerStore {
    fn query<'a>(
        &'a self,
        site: &'a Site,
        spec: &'a QuerySpec,
        now_ms: i64,
    ) -> BoxFuture<'a, Result<QueryResult, CoreError>> {
        let s = Arc::clone(&self.0);
        let site = site.clone();
        let spec = spec.clone();
        Box::pin(run_blocking(move || s.query_sync(&site, &spec, now_ms)))
    }

    fn realtime<'a>(
        &'a self,
        site: &'a Site,
        now_ms: i64,
    ) -> BoxFuture<'a, Result<RealtimeSnapshot, CoreError>> {
        let s = Arc::clone(&self.0);
        let site = site.clone();
        Box::pin(run_blocking(move || s.realtime_sync(&site, now_ms)))
    }

    fn purge_before(
        &self,
        site_id: SiteId,
        before_ms: i64,
    ) -> BoxFuture<'_, Result<u64, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || {
            s.purge_before_sync(site_id, before_ms)
        }))
    }
}

impl AnalyticsDataPlane for DuckDbServerStore {
    fn data_plane_capabilities(&self) -> StorageCapabilities {
        self.0.capabilities_sync()
    }

    fn flush_shutdown(&self) -> BoxFuture<'_, Result<(), CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.checkpoint()))
    }
}

async fn run_blocking<T, F>(f: F) -> Result<T, CoreError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CoreError> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|err| CoreError::Storage(format!("blocking duckdb task failed: {err}")))?
}
