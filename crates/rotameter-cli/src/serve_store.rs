#![forbid(unsafe_code)]
//! `SqliteStore` adapter implementing [`rotameter_server::ServerStore`].

use std::path::Path;

use rotameter_core::error::CoreError;
use rotameter_core::id::{KeyId, OrgId, SiteId, SiteRef};
use rotameter_core::privacy::VisitorId;
use rotameter_core::query::QuerySpec;
use rotameter_core::session::SessionSnapshot;
use rotameter_core::storage::{
    AnalyticsQueryStore, ControlPlaneStore, EventIngestStore, QueryResult, RealtimeSnapshot,
    StorageCapabilities, WriteBatch,
};
use rotameter_core::types::{ApiKeyRecord, Org, Site};
use rotameter_server::ServerStore;
use rotameter_store_sqlite::{SqliteConfig, SqliteStore, format_query_result_csv};

/// Wrapper bridging [`SqliteStore`] into `rotameter-server`'s [`ServerStore`] trait without
/// creating a workspace dependency from `rotameter-server` onto `rusqlite`.
pub struct SqliteServerStore(SqliteStore);

impl SqliteServerStore {
    /// Opens a file-backed `SQLite` WAL store at `path` for server mode.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if opening or migrating the database fails.
    pub fn open(path: impl AsRef<Path>, now_ms: i64) -> Result<Self, CoreError> {
        Ok(Self(SqliteStore::open(
            path,
            SqliteConfig::default(),
            now_ms,
        )?))
    }

    /// Opens an isolated in-memory `SQLite` store (useful for integration testing).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if initialization fails.
    pub fn open_in_memory(now_ms: i64) -> Result<Self, CoreError> {
        Ok(Self(SqliteStore::open_in_memory(
            SqliteConfig::default(),
            now_ms,
        )?))
    }
}

impl ControlPlaneStore for SqliteServerStore {
    fn capabilities(&self) -> StorageCapabilities {
        self.0.capabilities()
    }

    fn ensure_default_org(&self, now_ms: i64) -> Result<Org, CoreError> {
        self.0.ensure_default_org(now_ms)
    }

    fn upsert_site(&self, site: &Site) -> Result<Site, CoreError> {
        self.0.upsert_site(site)
    }

    fn resolve_site(&self, org_id: OrgId, site_ref: &SiteRef) -> Result<Option<Site>, CoreError> {
        self.0.resolve_site(org_id, site_ref)
    }

    fn list_sites(&self, org_id: OrgId) -> Result<Vec<Site>, CoreError> {
        self.0.list_sites(org_id)
    }

    fn delete_site(&self, site_id: SiteId) -> Result<bool, CoreError> {
        self.0.delete_site(site_id)
    }

    fn create_api_key(&self, record: &ApiKeyRecord) -> Result<(), CoreError> {
        self.0.create_api_key(record)
    }

    fn find_active_key_by_hash(
        &self,
        key_hash: &[u8; 32],
    ) -> Result<Option<ApiKeyRecord>, CoreError> {
        self.0.find_active_key_by_hash(key_hash)
    }

    fn list_api_keys(&self, org_id: OrgId) -> Result<Vec<ApiKeyRecord>, CoreError> {
        self.0.list_api_keys(org_id)
    }

    fn revoke_api_key(&self, key_id: KeyId, revoked_at_ms: i64) -> Result<bool, CoreError> {
        self.0.revoke_api_key(key_id, revoked_at_ms)
    }
}

impl EventIngestStore for SqliteServerStore {
    fn lookup_active_session(
        &self,
        site_id: SiteId,
        visitor_id: VisitorId,
        min_last_event_ms: i64,
    ) -> Result<Option<SessionSnapshot>, CoreError> {
        self.0
            .lookup_active_session(site_id, visitor_id, min_last_event_ms)
    }

    fn write_batch(&self, batch: &WriteBatch) -> Result<(), CoreError> {
        self.0.write_batch(batch)
    }
}

impl AnalyticsQueryStore for SqliteServerStore {
    fn query(&self, site: &Site, spec: &QuerySpec, now_ms: i64) -> Result<QueryResult, CoreError> {
        self.0.query(site, spec, now_ms)
    }

    fn realtime(&self, site: &Site, now_ms: i64) -> Result<RealtimeSnapshot, CoreError> {
        self.0.realtime(site, now_ms)
    }

    fn purge_before(&self, site_id: SiteId, before_ms: i64) -> Result<u64, CoreError> {
        self.0.purge_before(site_id, before_ms)
    }
}

impl ServerStore for SqliteServerStore {
    fn schema_version(&self) -> Result<u32, CoreError> {
        self.0.schema_version()
    }

    fn checkpoint_truncate(&self) -> Result<(), CoreError> {
        self.0.checkpoint_truncate()
    }

    fn format_csv(&self, result: &QueryResult, spec: &QuerySpec) -> String {
        format_query_result_csv(result, spec)
    }
}
