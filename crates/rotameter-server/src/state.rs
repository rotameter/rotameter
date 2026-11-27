#![forbid(unsafe_code)]
//! Shared server state, [`ServerStores`] pair, `CSPRNG` ID generation, and key bootstrap.

use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use chrono::DateTime;
use rotameter_core::error::CoreError;
use rotameter_core::id::{KeyId, OrgId, SiteId};
use rotameter_core::privacy::{SessionId, compute_api_key_hash};
use rotameter_core::storage::{AnalyticsDataPlane, ControlPlaneStore};
use rotameter_core::types::{ApiKeyRecord, KeyScope};

use crate::auth_config::UiConfig;
use crate::auth_session::OperatorAuthState;
use crate::config::{ServerConfig, ServerError, ServerRole};
use crate::geoip::GeoIpResolver;
use crate::handlers::admin_backup::LiveBackupExporter;
use crate::ingest::IngestPipeline;
use crate::rate_limit::IpRateLimiter;
use crate::salt_store::ServerSaltStore;
use crate::self_site::ensure_self_site;
use crate::ui_assets::validate_ui_config;

/// Split OLTP control-plane and OLAP analytics data-plane stores wired into `rotameter-server`.
#[derive(Clone)]
pub struct ServerStores {
    /// Transactional OLTP control plane (`orgs`, `sites`, `api_keys`).
    pub control: Arc<dyn ControlPlaneStore>,
    /// Analytical OLAP data plane (`events`, `sessions`, `rollups`, queries).
    pub analytics: Arc<dyn AnalyticsDataPlane>,
    /// Optional online portable `.tar.zst` exporter for `POST /v1/admin/backup`.
    pub backup_exporter: Option<Arc<dyn LiveBackupExporter>>,
}

impl ServerStores {
    /// Creates a [`ServerStores`] pair backed by a single unified store implementing both traits.
    pub fn unified<T>(store: Arc<T>) -> Self
    where
        T: ControlPlaneStore + AnalyticsDataPlane + 'static,
    {
        Self {
            control: store.clone(),
            analytics: store,
            backup_exporter: None,
        }
    }

    /// Creates a [`ServerStores`] pair from distinct control-plane and analytics stores.
    pub fn split(
        control: Arc<dyn ControlPlaneStore>,
        analytics: Arc<dyn AnalyticsDataPlane>,
    ) -> Self {
        Self {
            control,
            analytics,
            backup_exporter: None,
        }
    }

    /// Attaches an online portable `.tar.zst` exporter for `POST /v1/admin/backup`.
    #[must_use]
    pub fn with_backup_exporter(mut self, exporter: Arc<dyn LiveBackupExporter>) -> Self {
        self.backup_exporter = Some(exporter);
        self
    }
}

/// Shared application state injected into every Axum route handler.
#[derive(Clone)]
pub struct AppState {
    pub(crate) role: ServerRole,
    pub(crate) control_plane: Arc<dyn ControlPlaneStore>,
    pub(crate) analytics: Arc<dyn AnalyticsDataPlane>,
    pub(crate) backup_exporter: Option<Arc<dyn LiveBackupExporter>>,
    pub(crate) salt_store: ServerSaltStore,
    pub(crate) rate_limiter: Arc<IpRateLimiter>,
    pub(crate) geoip: Arc<GeoIpResolver>,
    /// Bounded asynchronous ingestion channel + batched writer task handle.
    pub ingest: IngestPipeline,
    pub(crate) default_org_id: OrgId,
    pub(crate) self_site_id: Option<SiteId>,
    pub(crate) mcp_http_enabled: bool,
    pub(crate) mcp_allow_admin: bool,
    pub(crate) operator_auth: Arc<OperatorAuthState>,
    pub(crate) ui: UiConfig,
    pub(crate) started_at: Instant,
}

impl AppState {
    /// Initializes shared server state, ensures the default org, and seeds an optional
    /// `ROTAMETER_BOOTSTRAP_ADMIN_KEY`.
    ///
    /// # Errors
    ///
    /// Returns [`ServerError`] if storage setup or bootstrap key validation fails.
    pub async fn new(stores: ServerStores, config: &ServerConfig) -> Result<Self, ServerError> {
        let now = now_ms();
        let org = stores
            .control
            .ensure_default_org(now)
            .await
            .map_err(|err| ServerError::new(format!("cannot ensure default org: {err}")))?;

        if let Some(raw_token) = config
            .bootstrap_admin_key
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            seed_bootstrap_admin_key(stores.control.as_ref(), org.org_id, raw_token, now).await?;
        }

        let self_site_id = if config.role.allows_ingest() {
            ensure_self_site(stores.control.as_ref(), org.org_id, config, now).await?
        } else {
            None
        };
        let geoip = Arc::new(GeoIpResolver::from_config(config)?);
        let rate_limiter = Arc::new(IpRateLimiter::new(config.rate_limit));
        let salt_store = ServerSaltStore::new(config.salt_store, stores.control.clone());
        salt_store
            .prune_expired_all(now)
            .await
            .map_err(|err| ServerError::new(format!("cannot prune expired salts: {err}")))?;
        validate_ui_config(&config.ui)?;
        let operator_auth = Arc::new(OperatorAuthState::from_config(config)?);
        let ingest = if config.role.allows_ingest() {
            IngestPipeline::spawn(stores.analytics.clone(), config)
        } else {
            IngestPipeline::disabled()
        };

        Ok(Self {
            role: config.role,
            control_plane: stores.control,
            analytics: stores.analytics,
            backup_exporter: stores.backup_exporter,
            salt_store,
            rate_limiter,
            geoip,
            ingest,
            default_org_id: org.org_id,
            self_site_id,
            mcp_http_enabled: config.mcp_http.enabled,
            mcp_allow_admin: config.mcp_http.allow_admin,
            operator_auth,
            ui: config.ui.clone(),
            started_at: Instant::now(),
        })
    }

    /// Returns the seeded local `self_site` UID when first-boot seeding is enabled.
    pub const fn self_site_id(&self) -> Option<SiteId> {
        self.self_site_id
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

async fn seed_bootstrap_admin_key(
    control: &dyn ControlPlaneStore,
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
    let existing = control
        .list_api_keys(org_id)
        .await
        .map_err(|err| ServerError::new(format!("cannot check existing API keys: {err}")))?;
    if existing.iter().any(|k| k.key_hash == key_hash) {
        return Ok(());
    }
    let key_id = new_key_id()
        .map_err(|err| ServerError::new(format!("cannot mint bootstrap key id: {err}")))?;
    let key_prefix: String = token.chars().take(12).collect();
    control
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
        .await
        .map_err(|err| ServerError::new(format!("cannot seed bootstrap admin key: {err}")))?;
    tracing::info!("seeded ROTAMETER_BOOTSTRAP_ADMIN_KEY for default organization");
    Ok(())
}
