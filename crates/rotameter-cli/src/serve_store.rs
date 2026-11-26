#![forbid(unsafe_code)]
//! Async `tokio::task::spawn_blocking` adapter bridging [`SqliteStore`] into
//! `rotameter-core` [`ControlPlaneStore`] and [`AnalyticsDataPlane`] traits.

use std::path::Path;
use std::sync::Arc;

use rotameter_core::error::CoreError;
use rotameter_core::id::{KeyId, OrgId, SiteId, SiteRef};
use rotameter_core::privacy::{SaltBytes, SaltWindow, VisitorId};
use rotameter_core::query::QuerySpec;
use rotameter_core::session::SessionSnapshot;
use rotameter_core::storage::{
    AnalyticsDataPlane, AnalyticsQueryStore, BoxFuture, ControlPlaneStore, EventIngestStore,
    QueryResult, RealtimeSnapshot, StorageCapabilities, WriteBatch,
};
use rotameter_core::types::{ApiKeyRecord, Org, SaltRotation, Site};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};

/// Thread-safe async wrapper around [`SqliteStore`] executing `SQLite` I/O on Tokio's
/// blocking thread pool without adding a `tokio` dependency to `rotameter-store-sqlite`.
#[derive(Clone)]
pub struct SqliteServerStore(Arc<SqliteStore>);

impl SqliteServerStore {
    /// Opens a file-backed `SQLite` WAL store at `path` for server mode.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if opening or migrating the database fails.
    pub fn open(path: impl AsRef<Path>, now_ms: i64) -> Result<Self, CoreError> {
        Ok(Self(Arc::new(SqliteStore::open(
            path,
            SqliteConfig::default(),
            now_ms,
        )?)))
    }

    /// Opens an isolated in-memory `SQLite` store (useful for integration testing).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if initialization fails.
    pub fn open_in_memory(now_ms: i64) -> Result<Self, CoreError> {
        Ok(Self(Arc::new(SqliteStore::open_in_memory(
            SqliteConfig::default(),
            now_ms,
        )?)))
    }
}

async fn run_blocking<T, F>(f: F) -> Result<T, CoreError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CoreError> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|err| CoreError::Storage(format!("blocking storage task failed: {err}")))?
}

impl ControlPlaneStore for SqliteServerStore {
    fn capabilities(&self) -> StorageCapabilities {
        self.0.capabilities_sync()
    }

    fn schema_version(&self) -> BoxFuture<'_, Result<u32, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.schema_version()))
    }

    fn flush_shutdown(&self) -> BoxFuture<'_, Result<(), CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.checkpoint_truncate()))
    }

    fn ensure_default_org(&self, now_ms: i64) -> BoxFuture<'_, Result<Org, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.ensure_default_org_sync(now_ms)))
    }

    fn upsert_site<'a>(&'a self, site: &'a Site) -> BoxFuture<'a, Result<Site, CoreError>> {
        let s = Arc::clone(&self.0);
        let site = site.clone();
        Box::pin(run_blocking(move || s.upsert_site_sync(&site)))
    }

    fn resolve_site<'a>(
        &'a self,
        org_id: OrgId,
        site_ref: &'a SiteRef,
    ) -> BoxFuture<'a, Result<Option<Site>, CoreError>> {
        let s = Arc::clone(&self.0);
        let site_ref = site_ref.clone();
        Box::pin(run_blocking(move || s.resolve_site_sync(org_id, &site_ref)))
    }

    fn list_sites(&self, org_id: OrgId) -> BoxFuture<'_, Result<Vec<Site>, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.list_sites_sync(org_id)))
    }

    fn delete_site(&self, site_id: SiteId) -> BoxFuture<'_, Result<bool, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.delete_site_sync(site_id)))
    }

    fn create_api_key<'a>(
        &'a self,
        record: &'a ApiKeyRecord,
    ) -> BoxFuture<'a, Result<(), CoreError>> {
        let s = Arc::clone(&self.0);
        let record = record.clone();
        Box::pin(run_blocking(move || s.create_api_key_sync(&record)))
    }

    fn find_active_key_by_hash<'a>(
        &'a self,
        key_hash: &'a [u8; 32],
    ) -> BoxFuture<'a, Result<Option<ApiKeyRecord>, CoreError>> {
        let s = Arc::clone(&self.0);
        let key_hash = *key_hash;
        Box::pin(run_blocking(move || {
            s.find_active_key_by_hash_sync(&key_hash)
        }))
    }

    fn list_api_keys(&self, org_id: OrgId) -> BoxFuture<'_, Result<Vec<ApiKeyRecord>, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.list_api_keys_sync(org_id)))
    }

    fn revoke_api_key(
        &self,
        key_id: KeyId,
        revoked_at_ms: i64,
    ) -> BoxFuture<'_, Result<bool, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || {
            s.revoke_api_key_sync(key_id, revoked_at_ms)
        }))
    }

    fn get_or_create_salt(
        &self,
        window: SaltWindow,
        candidate: SaltBytes,
        now_ms: i64,
    ) -> BoxFuture<'_, Result<SaltBytes, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || {
            s.get_or_create_salt_sync(window, candidate, now_ms)
        }))
    }

    fn prune_salts_before(
        &self,
        rotation: SaltRotation,
        min_window_id: i64,
    ) -> BoxFuture<'_, Result<u64, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || {
            s.prune_salts_before_sync(rotation, min_window_id)
        }))
    }
}

impl EventIngestStore for SqliteServerStore {
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

impl AnalyticsQueryStore for SqliteServerStore {
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

impl AnalyticsDataPlane for SqliteServerStore {
    fn data_plane_capabilities(&self) -> StorageCapabilities {
        self.0.capabilities_sync()
    }

    fn flush_shutdown(&self) -> BoxFuture<'_, Result<(), CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.checkpoint_truncate()))
    }
}
