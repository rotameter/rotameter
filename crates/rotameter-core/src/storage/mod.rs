#![forbid(unsafe_code)]
//! Pluggable storage trait boundaries (`ControlPlaneStore`, `EventIngestStore`, `AnalyticsQueryStore`).

pub mod ingest;
pub mod models;

use crate::error::CoreError;
use crate::id::{KeyId, OrgId, SiteId, SiteRef};
use crate::privacy::VisitorId;
use crate::query::QuerySpec;
use crate::session::SessionSnapshot;
use crate::types::{ApiKeyRecord, Org, Site};

pub use ingest::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
pub use models::{
    BreakdownRow, KpiSummary, QueryMeta, QueryResult, RealtimePage, RealtimePulse,
    RealtimeSnapshot, StorageCapabilities, TimeseriesPoint,
};

/// Control-plane persistence contract for organizations, sites, and hashed API keys.
pub trait ControlPlaneStore: Send + Sync {
    /// Returns the backend's declared [`StorageCapabilities`].
    fn capabilities(&self) -> StorageCapabilities;

    /// Ensures the local default organization (`slug = "default"`) exists and returns it.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    fn ensure_default_org(&self, now_ms: i64) -> Result<Org, CoreError>;

    /// Creates or updates a site idempotently.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on constraint or persistence failure.
    fn upsert_site(&self, site: &Site) -> Result<Site, CoreError>;

    /// Resolves a site by immutable UID or org-unique slug (`SiteRef`).
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    fn resolve_site(&self, org_id: OrgId, site_ref: &SiteRef) -> Result<Option<Site>, CoreError>;

    /// Lists all sites owned by `org_id`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    fn list_sites(&self, org_id: OrgId) -> Result<Vec<Site>, CoreError>;

    /// Deletes a site and cascades its associated analytics data.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    fn delete_site(&self, site_id: SiteId) -> Result<bool, CoreError>;

    /// Persists metadata and `SHA-256` hash for a newly minted API key.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    fn create_api_key(&self, record: &ApiKeyRecord) -> Result<(), CoreError>;

    /// Looks up an active (non-revoked) API key record by `SHA-256` token digest.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    fn find_active_key_by_hash(
        &self,
        key_hash: &[u8; 32],
    ) -> Result<Option<ApiKeyRecord>, CoreError>;

    /// Lists non-sensitive metadata for API keys owned by `org_id`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    fn list_api_keys(&self, org_id: OrgId) -> Result<Vec<ApiKeyRecord>, CoreError>;

    /// Revokes an API key at `revoked_at_ms`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on persistence failure.
    fn revoke_api_key(&self, key_id: KeyId, revoked_at_ms: i64) -> Result<bool, CoreError>;
}

/// Batched event ingestion persistence contract used by the dedicated writer task.
pub trait EventIngestStore: Send + Sync {
    /// Looks up the latest active session for `(site_id, visitor_id)` after LRU eviction.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query failure.
    fn lookup_active_session(
        &self,
        site_id: SiteId,
        visitor_id: VisitorId,
        min_last_event_ms: i64,
    ) -> Result<Option<SessionSnapshot>, CoreError>;

    /// Atomically persists events, session upserts, and rollup deltas in one transaction.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if the batch transaction fails to commit.
    fn write_batch(&self, batch: &WriteBatch) -> Result<(), CoreError>;
}

/// Read-only analytics query and retention purge persistence contract.
pub trait AnalyticsQueryStore: Send + Sync {
    /// Executes a validated [`QuerySpec`] against stored sessions, events, and rollups.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidQuery`] or [`CoreError::Storage`] on failure.
    fn query(&self, site: &Site, spec: &QuerySpec, now_ms: i64) -> Result<QueryResult, CoreError>;

    /// Reads the trailing 5-minute active snapshot and 30-minute pulse for `site`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query failure.
    fn realtime(&self, site: &Site, now_ms: i64) -> Result<RealtimeSnapshot, CoreError>;

    /// Deletes raw and/or rollup analytics for `site_id` strictly older than `before_ms`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on purge failure.
    fn purge_before(&self, site_id: SiteId, before_ms: i64) -> Result<u64, CoreError>;
}
