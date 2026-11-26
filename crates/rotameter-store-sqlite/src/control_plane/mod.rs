#![forbid(unsafe_code)]
//! `ControlPlaneStore` implementation for [`SqliteStore`].

mod keys;
mod sites;

use rotameter_core::error::CoreError;
use rotameter_core::id::{KeyId, OrgId, SiteId, SiteRef};
use rotameter_core::storage::{BoxFuture, ControlPlaneStore, StorageCapabilities};
use rotameter_core::types::{ApiKeyRecord, Org, Site};

use crate::store::SqliteStore;

impl SqliteStore {
    /// Returns the `SQLite` backend's declared [`StorageCapabilities`].
    pub const fn capabilities_sync(&self) -> StorageCapabilities {
        StorageCapabilities {
            supports_raw_event_log: true,
            supports_arbitrary_distinct_visitors: true,
            supports_multi_node_concurrent_writes: false,
            supports_sql_funnels: false,
            supports_raw_append_only: false,
            supports_control_plane: true,
            supports_analytics_data_plane: true,
        }
    }

    /// Synchronously ensures the local default organization exists and returns it.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    pub fn ensure_default_org_sync(&self, now_ms: i64) -> Result<Org, CoreError> {
        self.with_writer(|state| sites::ensure_default_org_row(&state.conn, now_ms))
    }

    /// Synchronously creates or updates a site idempotently.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on constraint or persistence failure.
    pub fn upsert_site_sync(&self, site: &Site) -> Result<Site, CoreError> {
        self.with_writer(|state| sites::upsert_site_row(&mut state.conn, site))
    }

    /// Synchronously resolves a site by immutable UID or org-unique slug.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    pub fn resolve_site_sync(
        &self,
        org_id: OrgId,
        site_ref: &SiteRef,
    ) -> Result<Option<Site>, CoreError> {
        self.with_reader(|conn| sites::resolve_site_row(conn, org_id, site_ref))
    }

    /// Synchronously lists all sites owned by `org_id`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    pub fn list_sites_sync(&self, org_id: OrgId) -> Result<Vec<Site>, CoreError> {
        self.with_reader(|conn| sites::list_site_rows(conn, org_id))
    }

    /// Synchronously deletes a site and cascades its associated rows.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    pub fn delete_site_sync(&self, site_id: SiteId) -> Result<bool, CoreError> {
        self.with_writer(|state| {
            sites::delete_site_cascade(&mut state.conn, &mut state.interner, site_id)
        })
    }

    /// Synchronously persists metadata and `SHA-256` hash for a newly minted API key.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    pub fn create_api_key_sync(&self, record: &ApiKeyRecord) -> Result<(), CoreError> {
        self.with_writer(|state| keys::insert_api_key(&state.conn, record))
    }

    /// Synchronously looks up an active (non-revoked) API key record by `SHA-256` token digest.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    pub fn find_active_key_by_hash_sync(
        &self,
        key_hash: &[u8; 32],
    ) -> Result<Option<ApiKeyRecord>, CoreError> {
        self.with_reader(|conn| keys::query_active_key_by_hash(conn, key_hash))
    }

    /// Synchronously lists non-sensitive metadata for API keys owned by `org_id`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    pub fn list_api_keys_sync(&self, org_id: OrgId) -> Result<Vec<ApiKeyRecord>, CoreError> {
        self.with_reader(|conn| keys::query_api_keys(conn, org_id))
    }

    /// Synchronously revokes an API key at `revoked_at_ms`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    pub fn revoke_api_key_sync(
        &self,
        key_id: KeyId,
        revoked_at_ms: i64,
    ) -> Result<bool, CoreError> {
        self.with_writer(|state| keys::mark_key_revoked(&state.conn, key_id, revoked_at_ms))
    }
}

impl ControlPlaneStore for SqliteStore {
    fn capabilities(&self) -> StorageCapabilities {
        self.capabilities_sync()
    }

    fn schema_version(&self) -> BoxFuture<'_, Result<u32, CoreError>> {
        Box::pin(std::future::ready(Self::schema_version(self)))
    }

    fn flush_shutdown(&self) -> BoxFuture<'_, Result<(), CoreError>> {
        Box::pin(std::future::ready(self.checkpoint_truncate()))
    }

    fn ensure_default_org(&self, now_ms: i64) -> BoxFuture<'_, Result<Org, CoreError>> {
        Box::pin(std::future::ready(self.ensure_default_org_sync(now_ms)))
    }

    fn upsert_site<'a>(&'a self, site: &'a Site) -> BoxFuture<'a, Result<Site, CoreError>> {
        Box::pin(std::future::ready(self.upsert_site_sync(site)))
    }

    fn resolve_site<'a>(
        &'a self,
        org_id: OrgId,
        site_ref: &'a SiteRef,
    ) -> BoxFuture<'a, Result<Option<Site>, CoreError>> {
        Box::pin(std::future::ready(self.resolve_site_sync(org_id, site_ref)))
    }

    fn list_sites(&self, org_id: OrgId) -> BoxFuture<'_, Result<Vec<Site>, CoreError>> {
        Box::pin(std::future::ready(self.list_sites_sync(org_id)))
    }

    fn delete_site(&self, site_id: SiteId) -> BoxFuture<'_, Result<bool, CoreError>> {
        Box::pin(std::future::ready(self.delete_site_sync(site_id)))
    }

    fn create_api_key<'a>(
        &'a self,
        record: &'a ApiKeyRecord,
    ) -> BoxFuture<'a, Result<(), CoreError>> {
        Box::pin(std::future::ready(self.create_api_key_sync(record)))
    }

    fn find_active_key_by_hash<'a>(
        &'a self,
        key_hash: &'a [u8; 32],
    ) -> BoxFuture<'a, Result<Option<ApiKeyRecord>, CoreError>> {
        Box::pin(std::future::ready(
            self.find_active_key_by_hash_sync(key_hash),
        ))
    }

    fn list_api_keys(&self, org_id: OrgId) -> BoxFuture<'_, Result<Vec<ApiKeyRecord>, CoreError>> {
        Box::pin(std::future::ready(self.list_api_keys_sync(org_id)))
    }

    fn revoke_api_key(
        &self,
        key_id: KeyId,
        revoked_at_ms: i64,
    ) -> BoxFuture<'_, Result<bool, CoreError>> {
        Box::pin(std::future::ready(
            self.revoke_api_key_sync(key_id, revoked_at_ms),
        ))
    }
}
