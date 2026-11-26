#![forbid(unsafe_code)]
//! Pluggable OLTP/OLAP storage trait boundaries (`ControlPlaneStore`, `AnalyticsDataPlane`).

pub mod csv;
pub mod exchange;
pub mod ingest;
pub mod models;

use std::future::Future;
use std::pin::Pin;

use crate::error::CoreError;
use crate::id::{KeyId, OrgId, SiteId, SiteRef};
use crate::privacy::{SaltBytes, SaltWindow, VisitorId};
use crate::query::QuerySpec;
use crate::session::SessionSnapshot;
use crate::types::{ApiKeyRecord, Org, SaltRotation, Site};

pub use csv::format_query_result_csv;
pub use exchange::{
    ControlPlaneSnapshot, RollupDailyRow, RollupHourlyRow, RollupPathDailyRow, RotatingSaltRecord,
};
pub use ingest::{IngestEvent, RollupDelta, SessionUpsert, WriteBatch};
pub use models::{
    BreakdownRow, KpiSummary, QueryMeta, QueryResult, RealtimePage, RealtimePulse,
    RealtimeSnapshot, StorageCapabilities, TimeseriesPoint,
};

/// Runtime-agnostic boxed future returned by async storage trait methods.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Control-plane persistence contract for organizations, sites, and hashed API keys.
pub trait ControlPlaneStore: Send + Sync {
    /// Returns the backend's declared [`StorageCapabilities`].
    fn capabilities(&self) -> StorageCapabilities;

    /// Returns the highest applied backend schema migration version.
    fn schema_version(&self) -> BoxFuture<'_, Result<u32, CoreError>>;

    /// Flushes or checkpoints control-plane storage on graceful shutdown.
    fn flush_shutdown(&self) -> BoxFuture<'_, Result<(), CoreError>> {
        Box::pin(std::future::ready(Ok(())))
    }

    /// Ensures the local default organization (`slug = "default"`) exists and returns it.
    fn ensure_default_org(&self, now_ms: i64) -> BoxFuture<'_, Result<Org, CoreError>>;

    /// Creates or updates a site idempotently.
    fn upsert_site<'a>(&'a self, site: &'a Site) -> BoxFuture<'a, Result<Site, CoreError>>;

    /// Resolves a site by immutable UID or org-unique slug (`SiteRef`).
    fn resolve_site<'a>(
        &'a self,
        org_id: OrgId,
        site_ref: &'a SiteRef,
    ) -> BoxFuture<'a, Result<Option<Site>, CoreError>>;

    /// Lists all sites owned by `org_id`.
    fn list_sites(&self, org_id: OrgId) -> BoxFuture<'_, Result<Vec<Site>, CoreError>>;

    /// Deletes a site and cascades its associated control-plane state.
    fn delete_site(&self, site_id: SiteId) -> BoxFuture<'_, Result<bool, CoreError>>;

    /// Persists metadata and `SHA-256` hash for a newly minted API key.
    fn create_api_key<'a>(
        &'a self,
        record: &'a ApiKeyRecord,
    ) -> BoxFuture<'a, Result<(), CoreError>>;

    /// Looks up an active (non-revoked) API key record by `SHA-256` token digest.
    fn find_active_key_by_hash<'a>(
        &'a self,
        key_hash: &'a [u8; 32],
    ) -> BoxFuture<'a, Result<Option<ApiKeyRecord>, CoreError>>;

    /// Lists non-sensitive metadata for API keys owned by `org_id`.
    fn list_api_keys(&self, org_id: OrgId) -> BoxFuture<'_, Result<Vec<ApiKeyRecord>, CoreError>>;

    /// Revokes an API key at `revoked_at_ms`.
    fn revoke_api_key(
        &self,
        key_id: KeyId,
        revoked_at_ms: i64,
    ) -> BoxFuture<'_, Result<bool, CoreError>>;

    /// Atomically fetches the persisted CSPRNG salt for `window` or inserts `candidate`.
    fn get_or_create_salt(
        &self,
        window: SaltWindow,
        candidate: SaltBytes,
        now_ms: i64,
    ) -> BoxFuture<'_, Result<SaltBytes, CoreError>>;

    /// Deletes persisted salt windows for `rotation` strictly older than `min_window_id`.
    fn prune_salts_before(
        &self,
        rotation: SaltRotation,
        min_window_id: i64,
    ) -> BoxFuture<'_, Result<u64, CoreError>>;
}

/// Batched event ingestion persistence contract used by the dedicated writer task.
pub trait EventIngestStore: Send + Sync {
    /// Looks up the latest active session for `(site_id, visitor_id)` after LRU eviction.
    fn lookup_active_session(
        &self,
        site_id: SiteId,
        visitor_id: VisitorId,
        min_last_event_ms: i64,
    ) -> BoxFuture<'_, Result<Option<SessionSnapshot>, CoreError>>;

    /// Atomically persists events, session upserts, and rollup deltas in one transaction.
    fn write_batch<'a>(&'a self, batch: &'a WriteBatch) -> BoxFuture<'a, Result<(), CoreError>>;
}

/// Read-only analytics query and retention purge persistence contract.
pub trait AnalyticsQueryStore: Send + Sync {
    /// Executes a validated [`QuerySpec`] against stored sessions, events, and rollups.
    fn query<'a>(
        &'a self,
        site: &'a Site,
        spec: &'a QuerySpec,
        now_ms: i64,
    ) -> BoxFuture<'a, Result<QueryResult, CoreError>>;

    /// Reads the trailing 5-minute active snapshot and 30-minute pulse for `site`.
    fn realtime<'a>(
        &'a self,
        site: &'a Site,
        now_ms: i64,
    ) -> BoxFuture<'a, Result<RealtimeSnapshot, CoreError>>;

    /// Deletes raw and/or rollup analytics for `site_id` strictly older than `before_ms`.
    fn purge_before(
        &self,
        site_id: SiteId,
        before_ms: i64,
    ) -> BoxFuture<'_, Result<u64, CoreError>>;
}

/// Combined OLAP analytics data-plane supertrait (ingest, query, realtime, retention).
pub trait AnalyticsDataPlane: EventIngestStore + AnalyticsQueryStore + Send + Sync {
    /// Returns the analytics data plane's declared [`StorageCapabilities`].
    fn data_plane_capabilities(&self) -> StorageCapabilities;

    /// Flushes buffered writes or checkpoints the analytics store on graceful shutdown.
    fn flush_shutdown(&self) -> BoxFuture<'_, Result<(), CoreError>> {
        Box::pin(std::future::ready(Ok(())))
    }
}
