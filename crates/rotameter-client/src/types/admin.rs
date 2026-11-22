#![forbid(unsafe_code)]
//! Wire DTOs for `/v1/health`, `/v1/admin/sites/*`, and `/v1/admin/keys/*`.

use rotameter_core::id::{KeyId, OrgId, SiteId, SiteRef};
use rotameter_core::types::{KeyScope, SaltRotation};
use serde::{Deserialize, Serialize};

/// Service liveness and ingest queue telemetry returned by `GET /v1/health`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HealthResponse {
    /// Server health status (`"ok"`).
    pub status: String,
    /// Compiled Rotameter workspace version.
    pub version: String,
    /// Applied backend database schema migration version.
    pub schema_version: u32,
    /// Current number of events queued in the ingest ring buffer.
    pub queue_depth: usize,
    /// Configured capacity of the ingest ring buffer.
    pub queue_capacity: usize,
    /// Ingest queue utilization percentage (`0.0..=100.0`).
    pub queue_fill_pct: f64,
    /// Process uptime in whole seconds.
    pub uptime_secs: u64,
}

/// Request payload for `POST /v1/admin/sites` and `PUT /v1/admin/sites/{site}`.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertSiteRequest {
    /// Org-unique site slug (`colinwolff.dev`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slug: Option<String>,
    /// Human-readable display label.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// IANA timezone name (defaults to `"UTC"`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    /// Visitor salt rotation cadence (`daily`, `weekly`, `monthly`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub salt_rotation: Option<SaltRotation>,
    /// Whether `Sec-GPC: 1` requests are ignored.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub respect_gpc: Option<bool>,
    /// Allowed browser origins/hosts for unauthenticated ingestion.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_origins: Option<Vec<String>>,
    /// Query parameter keys preserved in normalized paths beyond `utm_*`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_query_keys: Option<Vec<String>>,
    /// Raw event/session retention period in days.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw_retention_days: Option<u32>,
}

/// Site metadata returned by `/v1/admin/sites/*`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteResponse {
    /// Immutable prefixed `UUIDv7` site identifier.
    pub site_id: SiteId,
    /// Owning organization identifier.
    pub org_id: OrgId,
    /// Org-unique site slug.
    pub slug: String,
    /// Mutable human-readable label.
    pub display_name: String,
    /// IANA timezone name.
    pub timezone: String,
    /// Visitor salt rotation cadence.
    pub salt_rotation: SaltRotation,
    /// Whether `Sec-GPC: 1` hits are dropped.
    pub respect_gpc: bool,
    /// Whether this site is the local first-boot `self_site`.
    pub is_self_site: bool,
    /// Allowed browser origins/hosts.
    pub allowed_origins: Vec<String>,
    /// Allowed query keys preserved in normalized paths.
    pub allowed_query_keys: Vec<String>,
    /// Optional raw data retention window in days.
    pub raw_retention_days: Option<u32>,
    /// Creation timestamp (`RFC 3339` UTC).
    pub created_at: String,
    /// Last update timestamp (`RFC 3339` UTC).
    pub updated_at: String,
}

/// Confirmation payload returned by `DELETE /v1/admin/sites/{site}?confirm=true`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeleteSiteResponse {
    /// Whether a site row was deleted.
    pub deleted: bool,
    /// Deleted site's immutable UID.
    pub site_id: SiteId,
}

/// Request payload for `POST /v1/admin/keys`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateKeyRequest {
    /// Authorization scope (`ingest`, `read`, or `admin`).
    pub scope: KeyScope,
    /// Target site reference (`Some` for `ingest`/`read`, `None` for `admin`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub site: Option<SiteRef>,
    /// Human operator label (`1..=128` characters).
    pub label: String,
}

/// Newly minted API key response returned once by `POST /v1/admin/keys`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreatedKeyResponse {
    /// Immutable key UID.
    pub key_id: KeyId,
    /// Owning organization UID.
    pub org_id: OrgId,
    /// Granted scope.
    pub scope: KeyScope,
    /// Bound site UID (`None` for org-wide admin keys).
    pub site_id: Option<SiteId>,
    /// Non-sensitive 12-character display prefix.
    pub key_prefix: String,
    /// Human operator label.
    pub label: String,
    /// Plaintext Bearer token shown only at creation time.
    pub token: String,
    /// Creation timestamp (`RFC 3339` UTC).
    pub created_at: String,
}

/// Non-sensitive key metadata returned by `GET /v1/admin/keys`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyMetadataResponse {
    /// Immutable key UID.
    pub key_id: KeyId,
    /// Owning organization UID.
    pub org_id: OrgId,
    /// Granted scope.
    pub scope: KeyScope,
    /// Bound site UID (`None` for org-wide admin keys).
    pub site_id: Option<SiteId>,
    /// Non-sensitive 12-character display prefix.
    pub key_prefix: String,
    /// Human operator label.
    pub label: String,
    /// Creation timestamp (`RFC 3339` UTC).
    pub created_at: String,
    /// Optional last-used timestamp (`RFC 3339` UTC).
    pub last_used_at: Option<String>,
    /// Optional revocation timestamp (`RFC 3339` UTC).
    pub revoked_at: Option<String>,
}

/// Confirmation payload returned by `DELETE /v1/admin/keys/{key_id}`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevokeKeyResponse {
    /// Whether the key transitioned to revoked state.
    pub revoked: bool,
    /// Revoked key's immutable UID.
    pub key_id: KeyId,
}
