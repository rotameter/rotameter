#![forbid(unsafe_code)]
//! `ControlPlaneStore` implementation for [`SqliteStore`].

mod keys;
mod sites;

use rotameter_core::error::CoreError;
use rotameter_core::id::{KeyId, OrgId, SiteId, SiteRef};
use rotameter_core::storage::{ControlPlaneStore, StorageCapabilities};
use rotameter_core::types::{ApiKeyRecord, Org, Site};

use crate::store::SqliteStore;

impl ControlPlaneStore for SqliteStore {
    fn capabilities(&self) -> StorageCapabilities {
        StorageCapabilities {
            supports_raw_event_log: true,
            supports_arbitrary_distinct_visitors: true,
            supports_multi_node_concurrent_writes: false,
            supports_sql_funnels: false,
        }
    }

    fn ensure_default_org(&self, now_ms: i64) -> Result<Org, CoreError> {
        self.with_writer(|state| sites::ensure_default_org_row(&state.conn, now_ms))
    }

    fn upsert_site(&self, site: &Site) -> Result<Site, CoreError> {
        self.with_writer(|state| sites::upsert_site_row(&mut state.conn, site))
    }

    fn resolve_site(&self, org_id: OrgId, site_ref: &SiteRef) -> Result<Option<Site>, CoreError> {
        self.with_reader(|conn| sites::resolve_site_row(conn, org_id, site_ref))
    }

    fn list_sites(&self, org_id: OrgId) -> Result<Vec<Site>, CoreError> {
        self.with_reader(|conn| sites::list_site_rows(conn, org_id))
    }

    fn delete_site(&self, site_id: SiteId) -> Result<bool, CoreError> {
        self.with_writer(|state| {
            sites::delete_site_cascade(&mut state.conn, &mut state.interner, site_id)
        })
    }

    fn create_api_key(&self, record: &ApiKeyRecord) -> Result<(), CoreError> {
        self.with_writer(|state| keys::insert_api_key(&state.conn, record))
    }

    fn find_active_key_by_hash(
        &self,
        key_hash: &[u8; 32],
    ) -> Result<Option<ApiKeyRecord>, CoreError> {
        self.with_reader(|conn| keys::query_active_key_by_hash(conn, key_hash))
    }

    fn list_api_keys(&self, org_id: OrgId) -> Result<Vec<ApiKeyRecord>, CoreError> {
        self.with_reader(|conn| keys::query_api_keys(conn, org_id))
    }

    fn revoke_api_key(&self, key_id: KeyId, revoked_at_ms: i64) -> Result<bool, CoreError> {
        self.with_writer(|state| keys::mark_key_revoked(&state.conn, key_id, revoked_at_ms))
    }
}
