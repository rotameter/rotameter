#![forbid(unsafe_code)]
//! Shared live `rotameter-server` fixture for `rotameter-client` integration tests.

use std::sync::Arc;

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
use rotameter_server::{AppState, ServerConfig, ServerStore, build_router};
use rotameter_store_sqlite::{SqliteConfig, SqliteStore, format_query_result_csv};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

pub(crate) struct TestStore(pub(crate) SqliteStore);

impl ControlPlaneStore for TestStore {
    fn capabilities(&self) -> StorageCapabilities {
        self.0.capabilities()
    }
    fn ensure_default_org(&self, n: i64) -> Result<Org, CoreError> {
        self.0.ensure_default_org(n)
    }
    fn upsert_site(&self, s: &Site) -> Result<Site, CoreError> {
        self.0.upsert_site(s)
    }
    fn resolve_site(&self, o: OrgId, r: &SiteRef) -> Result<Option<Site>, CoreError> {
        self.0.resolve_site(o, r)
    }
    fn list_sites(&self, o: OrgId) -> Result<Vec<Site>, CoreError> {
        self.0.list_sites(o)
    }
    fn delete_site(&self, id: SiteId) -> Result<bool, CoreError> {
        self.0.delete_site(id)
    }
    fn create_api_key(&self, r: &ApiKeyRecord) -> Result<(), CoreError> {
        self.0.create_api_key(r)
    }
    fn find_active_key_by_hash(&self, h: &[u8; 32]) -> Result<Option<ApiKeyRecord>, CoreError> {
        self.0.find_active_key_by_hash(h)
    }
    fn list_api_keys(&self, o: OrgId) -> Result<Vec<ApiKeyRecord>, CoreError> {
        self.0.list_api_keys(o)
    }
    fn revoke_api_key(&self, id: KeyId, ts: i64) -> Result<bool, CoreError> {
        self.0.revoke_api_key(id, ts)
    }
}

impl EventIngestStore for TestStore {
    fn lookup_active_session(
        &self,
        s: SiteId,
        v: VisitorId,
        m: i64,
    ) -> Result<Option<SessionSnapshot>, CoreError> {
        self.0.lookup_active_session(s, v, m)
    }
    fn write_batch(&self, b: &WriteBatch) -> Result<(), CoreError> {
        self.0.write_batch(b)
    }
}

impl AnalyticsQueryStore for TestStore {
    fn query(&self, s: &Site, q: &QuerySpec, n: i64) -> Result<QueryResult, CoreError> {
        self.0.query(s, q, n)
    }
    fn realtime(&self, s: &Site, n: i64) -> Result<RealtimeSnapshot, CoreError> {
        self.0.realtime(s, n)
    }
    fn purge_before(&self, s: SiteId, b: i64) -> Result<u64, CoreError> {
        self.0.purge_before(s, b)
    }
}

impl ServerStore for TestStore {
    fn schema_version(&self) -> Result<u32, CoreError> {
        self.0.schema_version()
    }
    fn checkpoint_truncate(&self) -> Result<(), CoreError> {
        self.0.checkpoint_truncate()
    }
    fn format_csv(&self, r: &QueryResult, q: &QuerySpec) -> String {
        format_query_result_csv(r, q)
    }
}

pub(crate) async fn spawn_live_server(
    bootstrap_admin_key: &str,
) -> Result<(String, AppState, JoinHandle<()>), Box<dyn std::error::Error + Send + Sync>> {
    let sqlite = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000)?;
    let store = Arc::new(TestStore(sqlite));
    let state = AppState::new(
        store,
        &ServerConfig {
            bootstrap_admin_key: Some(bootstrap_admin_key.to_string()),
            ..ServerConfig::default()
        },
    )?;
    let router = build_router(state.clone());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });
    Ok((format!("http://{addr}"), state, handle))
}
