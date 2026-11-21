#![forbid(unsafe_code)]
//! Shared server state, [`ServerStore`] trait, `CSPRNG` ID generation, and key bootstrap.

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::DateTime;
use rotameter_core::error::CoreError;
use rotameter_core::id::{KeyId, OrgId, SiteId};
use rotameter_core::privacy::{
    InMemorySaltProvider, SaltBytes, SaltProvider, SessionId, compute_api_key_hash,
};
use rotameter_core::query::QuerySpec;
use rotameter_core::storage::{
    AnalyticsQueryStore, ControlPlaneStore, EventIngestStore, QueryResult,
};
use rotameter_core::types::{ApiKeyRecord, KeyScope};

use crate::config::{ServerConfig, ServerError};
use crate::ingest::IngestPipeline;

/// Unified persistence trait required by `rotameter-server`.
pub trait ServerStore:
    ControlPlaneStore + EventIngestStore + AnalyticsQueryStore + Send + Sync + 'static
{
    /// Returns the highest applied backend schema migration version.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if inspecting schema state fails.
    fn schema_version(&self) -> Result<u32, CoreError>;

    /// Flushes/truncates the storage write-ahead log on graceful shutdown.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] if the checkpoint fails.
    fn checkpoint_truncate(&self) -> Result<(), CoreError>;

    /// Formats a [`QueryResult`] into an `RFC 4180` CSV payload according to `spec`.
    fn format_csv(&self, result: &QueryResult, spec: &QuerySpec) -> String;
}

/// Shared application state injected into every Axum route handler.
#[derive(Clone)]
pub struct AppState {
    pub(crate) store: Arc<dyn ServerStore>,
    pub(crate) salt_provider: Arc<dyn SaltProvider>,
    /// Bounded asynchronous ingestion channel + batched writer task handle.
    pub ingest: IngestPipeline,
    pub(crate) default_org_id: OrgId,
    pub(crate) started_at: Instant,
}

impl AppState {
    /// Initializes shared server state, ensures the default org, and seeds an optional
    /// `ROTAMETER_BOOTSTRAP_ADMIN_KEY`.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if storage setup or bootstrap key validation fails.
    pub fn new(store: Arc<dyn ServerStore>, config: &ServerConfig) -> Result<Self, ServerError> {
        let now = now_ms();
        let org = store
            .ensure_default_org(now)
            .map_err(|err| ServerError::new(format!("cannot ensure default org: {err}")))?;

        if let Some(raw_token) = config
            .bootstrap_admin_key
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            seed_bootstrap_admin_key(store.as_ref(), org.org_id, raw_token, now)?;
        }

        let salt_provider: Arc<dyn SaltProvider> =
            Arc::new(InMemorySaltProvider::new(|_| random_salt_or_fallback()));
        let ingest = IngestPipeline::spawn(store.clone(), config);

        Ok(Self {
            store,
            salt_provider,
            ingest,
            default_org_id: org.org_id,
            started_at: Instant::now(),
        })
    }
}

pub(crate) fn now_ms() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
}

pub(crate) fn fill_random<const N: usize>() -> Result<[u8; N], CoreError> {
    let mut buf = [0_u8; N];
    getrandom::fill(&mut buf)
        .map_err(|err| CoreError::SaltUnavailable(format!("CSPRNG failure: {err}")))?;
    Ok(buf)
}

pub(crate) fn new_site_id() -> Result<SiteId, CoreError> {
    let ts = u64::try_from(now_ms().max(0)).unwrap_or(0);
    Ok(SiteId::from_uuidv7_parts(ts, fill_random::<10>()?))
}

pub(crate) fn new_key_id() -> Result<KeyId, CoreError> {
    let ts = u64::try_from(now_ms().max(0)).unwrap_or(0);
    Ok(KeyId::from_uuidv7_parts(ts, fill_random::<10>()?))
}

pub(crate) fn new_session_id() -> SessionId {
    fill_random::<16>().unwrap_or_else(|_| {
        let fallback = now_ms().to_be_bytes();
        let mut out = [0_u8; 16];
        out[..8].copy_from_slice(&fallback);
        out
    })
}

pub(crate) fn format_rfc3339(ts_ms: i64) -> String {
    DateTime::from_timestamp_millis(ts_ms).map_or_else(
        || "1970-01-01T00:00:00Z".to_string(),
        |dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    )
}

fn random_salt_or_fallback() -> SaltBytes {
    fill_random::<32>().unwrap_or_else(|_| {
        let now = now_ms().to_be_bytes();
        let mut out = [0_u8; 32];
        out[..8].copy_from_slice(&now);
        out
    })
}

fn seed_bootstrap_admin_key(
    store: &dyn ServerStore,
    org_id: OrgId,
    token: &str,
    now_ms: i64,
) -> Result<(), ServerError> {
    let prefix = KeyScope::Admin.token_prefix();
    if !token.starts_with(prefix) || token.len() < 16 {
        return Err(ServerError::new(format!(
            "ROTAMETER_BOOTSTRAP_ADMIN_KEY must start with `{prefix}` and be >= 16 characters"
        )));
    }
    let key_hash = compute_api_key_hash(token);
    let existing = store
        .list_api_keys(org_id)
        .map_err(|err| ServerError::new(format!("cannot check existing API keys: {err}")))?;
    if existing.iter().any(|k| k.key_hash == key_hash) {
        return Ok(());
    }
    let key_id = new_key_id()
        .map_err(|err| ServerError::new(format!("cannot mint bootstrap key id: {err}")))?;
    let key_prefix: String = token.chars().take(12).collect();
    store
        .create_api_key(&ApiKeyRecord {
            key_id,
            org_id,
            key_prefix,
            key_hash,
            scope: KeyScope::Admin,
            site_id: None,
            label: "bootstrap-admin".to_string(),
            created_at_ms: now_ms,
            last_used_ms: None,
            revoked_at_ms: None,
        })
        .map_err(|err| ServerError::new(format!("cannot seed bootstrap admin key: {err}")))?;
    tracing::info!("seeded ROTAMETER_BOOTSTRAP_ADMIN_KEY for default organization");
    Ok(())
}
