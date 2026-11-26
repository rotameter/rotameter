#![forbid(unsafe_code)]
//! Shared live `rotameter-server` fixture for `rotameter-client` integration tests.

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
use rotameter_server::{AppState, ServerConfig, ServerStores, build_router};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub(crate) struct TestStore(pub(crate) Arc<SqliteStore>);

async fn run_blocking<T, F>(f: F) -> Result<T, CoreError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CoreError> + Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|err| CoreError::Storage(format!("blocking storage task failed: {err}")))?
}

impl ControlPlaneStore for TestStore {
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
    fn ensure_default_org(&self, n: i64) -> BoxFuture<'_, Result<Org, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.ensure_default_org_sync(n)))
    }
    fn upsert_site<'a>(&'a self, site: &'a Site) -> BoxFuture<'a, Result<Site, CoreError>> {
        let s = Arc::clone(&self.0);
        let site = site.clone();
        Box::pin(run_blocking(move || s.upsert_site_sync(&site)))
    }
    fn resolve_site<'a>(
        &'a self,
        o: OrgId,
        r: &'a SiteRef,
    ) -> BoxFuture<'a, Result<Option<Site>, CoreError>> {
        let s = Arc::clone(&self.0);
        let r = r.clone();
        Box::pin(run_blocking(move || s.resolve_site_sync(o, &r)))
    }
    fn list_sites(&self, o: OrgId) -> BoxFuture<'_, Result<Vec<Site>, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.list_sites_sync(o)))
    }
    fn delete_site(&self, id: SiteId) -> BoxFuture<'_, Result<bool, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.delete_site_sync(id)))
    }
    fn create_api_key<'a>(&'a self, r: &'a ApiKeyRecord) -> BoxFuture<'a, Result<(), CoreError>> {
        let s = Arc::clone(&self.0);
        let r = r.clone();
        Box::pin(run_blocking(move || s.create_api_key_sync(&r)))
    }
    fn find_active_key_by_hash<'a>(
        &'a self,
        h: &'a [u8; 32],
    ) -> BoxFuture<'a, Result<Option<ApiKeyRecord>, CoreError>> {
        let s = Arc::clone(&self.0);
        let h = *h;
        Box::pin(run_blocking(move || s.find_active_key_by_hash_sync(&h)))
    }
    fn list_api_keys(&self, o: OrgId) -> BoxFuture<'_, Result<Vec<ApiKeyRecord>, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.list_api_keys_sync(o)))
    }
    fn revoke_api_key(&self, id: KeyId, ts: i64) -> BoxFuture<'_, Result<bool, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.revoke_api_key_sync(id, ts)))
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

impl EventIngestStore for TestStore {
    fn lookup_active_session(
        &self,
        site_id: SiteId,
        v: VisitorId,
        m: i64,
    ) -> BoxFuture<'_, Result<Option<SessionSnapshot>, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || {
            s.lookup_active_session_sync(site_id, v, m)
        }))
    }
    fn write_batch<'a>(&'a self, b: &'a WriteBatch) -> BoxFuture<'a, Result<(), CoreError>> {
        let s = Arc::clone(&self.0);
        let b = b.clone();
        Box::pin(run_blocking(move || s.write_batch_sync(&b)))
    }
}

impl AnalyticsQueryStore for TestStore {
    fn query<'a>(
        &'a self,
        site: &'a Site,
        q: &'a QuerySpec,
        n: i64,
    ) -> BoxFuture<'a, Result<QueryResult, CoreError>> {
        let s = Arc::clone(&self.0);
        let site = site.clone();
        let q = q.clone();
        Box::pin(run_blocking(move || s.query_sync(&site, &q, n)))
    }
    fn realtime<'a>(
        &'a self,
        site: &'a Site,
        n: i64,
    ) -> BoxFuture<'a, Result<RealtimeSnapshot, CoreError>> {
        let s = Arc::clone(&self.0);
        let site = site.clone();
        Box::pin(run_blocking(move || s.realtime_sync(&site, n)))
    }
    fn purge_before(&self, site_id: SiteId, b: i64) -> BoxFuture<'_, Result<u64, CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.purge_before_sync(site_id, b)))
    }
}

impl AnalyticsDataPlane for TestStore {
    fn data_plane_capabilities(&self) -> StorageCapabilities {
        self.0.capabilities_sync()
    }
    fn flush_shutdown(&self) -> BoxFuture<'_, Result<(), CoreError>> {
        let s = Arc::clone(&self.0);
        Box::pin(run_blocking(move || s.checkpoint_truncate()))
    }
}

pub(crate) async fn spawn_live_server(
    bootstrap_admin_key: &str,
) -> Result<(String, AppState, JoinHandle<()>), Box<dyn std::error::Error + Send + Sync>> {
    let sqlite = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000)?;
    let store = Arc::new(TestStore(Arc::new(sqlite)));
    let state = AppState::new(
        ServerStores::unified(store),
        &ServerConfig {
            bootstrap_admin_key: Some(bootstrap_admin_key.to_string()),
            self_site_enabled: false,
            ..ServerConfig::default()
        },
    )
    .await?;
    let router = build_router(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Ok((format!("http://{addr}"), state, handle))
}
