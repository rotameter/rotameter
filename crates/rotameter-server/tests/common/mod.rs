#![forbid(unsafe_code)]
//! Shared `SqliteStore`-backed `ServerStore` fixture and HTTP test client helpers.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Method, Request, StatusCode, header};
use http_body_util::BodyExt;
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
use serde_json::Value;
use tower::ServiceExt;

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

#[allow(dead_code)]
pub(crate) fn in_memory_app(bootstrap_admin_key: &str) -> (AppState, Router) {
    let Ok(sqlite) = SqliteStore::open_in_memory(SqliteConfig::default(), 1_700_000_000_000) else {
        std::process::abort()
    };
    let store = Arc::new(TestStore(sqlite));
    let Ok(state) = AppState::new(
        store,
        &ServerConfig {
            bootstrap_admin_key: Some(bootstrap_admin_key.to_string()),
            ..ServerConfig::default()
        },
    ) else {
        std::process::abort()
    };
    let app = build_router(state.clone());
    (state, app)
}

pub(crate) async fn request(
    app: &Router,
    method: Method,
    uri: &str,
    tok: Option<&str>,
    extra: &[(&str, &str)],
    body: Option<&Value>,
) -> (StatusCode, Vec<u8>) {
    let mut b = Request::builder().method(method).uri(uri);
    if let Some(t) = tok {
        b = b.header(header::AUTHORIZATION, format!("Bearer {t}"));
    }
    for (k, v) in extra {
        b = b.header(*k, *v);
    }
    let built = match body {
        Some(j) => b
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from(j.to_string())),
        None => b.body(Body::empty()),
    };
    let Ok(req) = built else {
        std::process::abort()
    };
    let Ok(resp) = app.clone().oneshot(req).await;
    let status = resp.status();
    let Ok(bytes) = resp.into_body().collect().await else {
        std::process::abort()
    };
    (status, bytes.to_bytes().to_vec())
}

pub(crate) fn as_json(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).unwrap_or(Value::Null)
}
