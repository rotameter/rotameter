#![forbid(unsafe_code)]
//! Runtime configuration and startup error types for `rotameter-server`.

use std::fmt;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::PathBuf;

use crate::auth_config::{OperatorAuthConfig, UiConfig};

/// Operational HTTP endpoint role served by a `rotameter serve` node.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ServerRole {
    /// Combined single-node server exposing ingest, query, admin, and health endpoints.
    #[default]
    All,
    /// Dedicated ingestion node exposing `/v1/health`, `/v1/collect.js`, `/v1/pixel.gif`, `/v1/events*`.
    Ingest,
    /// Dedicated read/admin node exposing `/v1/health`, `/v1/query`, `/v1/sites/*/realtime`, `/v1/admin/*`.
    Query,
}

impl ServerRole {
    /// Returns the canonical lowercase CLI/wire role name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Ingest => "ingest",
            Self::Query => "query",
        }
    }

    /// Returns `true` when this role serves event collection and runs `IngestPipeline`.
    pub const fn allows_ingest(self) -> bool {
        matches!(self, Self::All | Self::Ingest)
    }

    /// Returns `true` when this role serves query and admin routes.
    pub const fn allows_query(self) -> bool {
        matches!(self, Self::All | Self::Query)
    }
}

/// Storage strategy for daily/weekly/monthly rotating CSPRNG visitor salts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SaltStoreMode {
    /// Persisted in `ControlPlaneStore` (`rotating_salts`) with in-memory caching & expiry pruning.
    #[default]
    Db,
    /// Ephemeral process-local in-memory rotating salts only.
    Memory,
}

/// `GeoIP` lookup mode configured for incoming request enrichment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum GeoIpMode {
    /// Trust proxy headers (`X-Rotameter-*` for authenticated requests, `CF-*` when trusted).
    #[default]
    Header,
    /// Resolve missing geo headers via a local memory-mapped `MaxMind` `.mmdb` database.
    Mmdb,
}

/// Per-IP token-bucket rate limiter settings for unauthenticated browser ingest/assets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RateLimitConfig {
    /// Whether per-IP rate limiting is enforced on unauthenticated ingest/collector endpoints.
    pub enabled: bool,
    /// Sustained refill rate per client IP (`60 req/min` default).
    pub requests_per_minute: u32,
    /// Maximum immediate burst capacity per client IP (`20` default).
    pub burst: u32,
    /// Hard cap on distinct client IPs tracked in memory (`10,000` default).
    pub max_tracked_ips: usize,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            requests_per_minute: 60,
            burst: 20,
            max_tracked_ips: 10_000,
        }
    }
}

/// Streamable HTTP `/mcp` server settings.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct McpHttpConfig {
    /// Whether the Streamable HTTP `/mcp` route is mounted (`false` by default).
    pub enabled: bool,
    /// Whether `/mcp` exposes guarded admin tools to `rm_adm_` tokens (`false` by default).
    pub allow_admin: bool,
}

/// HTTP server and bounded ingestion pipeline configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerConfig {
    /// Socket address to bind the HTTP listener (`127.0.0.1:8000` by default).
    pub bind_addr: SocketAddr,
    /// Server endpoint role (`all`, `ingest`, or `query`; `all` by default).
    pub role: ServerRole,
    /// Rotating salt storage backend (`db` by default, or `memory`).
    pub salt_store: SaltStoreMode,
    /// Optional plaintext admin token (`rm_adm_...`) idempotently seeded on startup.
    pub bootstrap_admin_key: Option<String>,
    /// Optional public server URL whose host becomes the default `self_site` slug.
    pub public_url: Option<String>,
    /// Whether first-boot initialization seeds the local `self_site` (`true` by default).
    pub self_site_enabled: bool,
    /// Optional explicit `site_...` UID or slug overriding the `self_site` identifier.
    pub self_site_id: Option<String>,
    /// Whether `CF-*` proxy headers are trusted on unauthenticated requests (`true` by default).
    pub trusted_proxy_headers: bool,
    /// `GeoIP` enrichment strategy (`header` or `mmdb`).
    pub geoip_mode: GeoIpMode,
    /// Filesystem path to `.mmdb` database when `geoip_mode == GeoIpMode::Mmdb`.
    pub mmdb_path: Option<PathBuf>,
    /// Per-IP token-bucket rate-limit policy for unauthenticated collection endpoints.
    pub rate_limit: RateLimitConfig,
    /// Capacity of the bounded in-memory ingestion channel (`16,384` events).
    pub queue_capacity: usize,
    /// Maximum wait before flushing a partial batch to storage (`1,000 ms`).
    pub flush_interval_ms: u64,
    /// Maximum queued events folded into a single `WriteBatch` transaction (`512`).
    pub max_batch_size: usize,
    /// Graceful `SIGTERM` / `SIGINT` drain timeout in seconds (`10 s`).
    pub shutdown_timeout_secs: u64,
    /// Streamable HTTP `/mcp` endpoint settings.
    pub mcp_http: McpHttpConfig,
    /// Operator session, OIDC, and Cloudflare Access settings.
    pub operator_auth: OperatorAuthConfig,
    /// Static Web UI serving settings (`--ui=on|off`, `--ui-dir`).
    pub ui: UiConfig,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8000),
            role: ServerRole::All,
            salt_store: SaltStoreMode::Db,
            bootstrap_admin_key: None,
            public_url: None,
            self_site_enabled: true,
            self_site_id: None,
            trusted_proxy_headers: true,
            geoip_mode: GeoIpMode::Header,
            mmdb_path: None,
            rate_limit: RateLimitConfig::default(),
            queue_capacity: 16_384,
            flush_interval_ms: 1_000,
            max_batch_size: 512,
            shutdown_timeout_secs: 10,
            mcp_http: McpHttpConfig::default(),
            operator_auth: OperatorAuthConfig::default(),
            ui: UiConfig::default(),
        }
    }
}

/// Startup or listener failure surfaced by [`crate::serve`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerError {
    message: String,
}

impl ServerError {
    /// Creates a new [`ServerError`] from a descriptive message.
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ServerError {}
