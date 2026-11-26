#![forbid(unsafe_code)]
//! Core domain entities and enumerations shared across all Rotameter crates.

pub mod dimensions;
pub mod event;

use serde::{Deserialize, Serialize};

use crate::id::{KeyId, OrgId, SiteId};

pub use dimensions::{Channel, DeviceType, DimensionKind, ViewportClass};
pub use event::{
    EventKind, EventProps, MAX_EVENT_NAME_BYTES, MAX_PATH_BYTES, MAX_PROP_KEY_BYTES,
    MAX_PROP_VALUE_BYTES, MAX_PROPS_KEYS, MAX_USER_AGENT_BYTES, MAX_UTM_BYTES, UtmParams,
};

/// Salt rotation period controlling daily/weekly/monthly visitor deduplication windows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SaltRotation {
    /// Rotate CSPRNG salt daily at `00:00 UTC` (default).
    #[default]
    Daily,
    /// Rotate CSPRNG salt at ISO Monday `00:00 UTC`.
    Weekly,
    /// Rotate CSPRNG salt on the 1st of each UTC month.
    Monthly,
}

impl SaltRotation {
    /// Returns the `SQLite` / wire text representation.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }

    /// Parses a `SQLite` / wire salt rotation name (`daily`, `weekly`, or `monthly`).
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().to_ascii_lowercase().as_str() {
            "daily" => Some(Self::Daily),
            "weekly" => Some(Self::Weekly),
            "monthly" => Some(Self::Monthly),
            _ => None,
        }
    }
}

/// Authorization scope attached to a hashed API key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyScope {
    /// Write-only event and batch ingestion for a single site.
    Ingest,
    /// Read-only analytics query and realtime access for a site.
    Read,
    /// Full control-plane and query access across an organization.
    Admin,
}

impl KeyScope {
    /// Returns the token prefix expected in Bearer headers (`rm_ing_`, `rm_rd_`, `rm_adm_`).
    pub const fn token_prefix(self) -> &'static str {
        match self {
            Self::Ingest => "rm_ing_",
            Self::Read => "rm_rd_",
            Self::Admin => "rm_adm_",
        }
    }
}

/// Tenant organization owning sites and API keys (`default` auto-seeded in self-hosted `v1.0`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Org {
    /// Immutable organization UID.
    pub org_id: OrgId,
    /// Globally unique URL-friendly handle.
    pub slug: String,
    /// Mutable human-readable label.
    pub display_name: String,
    /// Creation timestamp in Unix milliseconds.
    pub created_at_ms: i64,
    /// Last update timestamp in Unix milliseconds.
    pub updated_at_ms: i64,
}

/// Registered tracked property belonging to an [`Org`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Site {
    /// Immutable site UID.
    pub site_id: SiteId,
    /// Owning organization UID.
    pub org_id: OrgId,
    /// Org-unique slug (e.g., `colinwolff.dev` or `_self`).
    pub slug: String,
    /// Mutable display label shown in UIs.
    pub display_name: String,
    /// IANA timezone name used for local day-boundary bucketing.
    pub timezone: String,
    /// Visitor salt rotation cadence.
    pub salt_rotation: SaltRotation,
    /// Whether `Sec-GPC: 1` requests are dropped.
    pub respect_gpc: bool,
    /// Whether this row is the local preconfigured `self_site`.
    pub is_self_site: bool,
    /// Allowed browser origins/hosts permitted to ingest events.
    pub allowed_origins: Vec<String>,
    /// Query parameter keys preserved in normalized paths beyond `utm_*`.
    pub allowed_query_keys: Vec<String>,
    /// Optional raw event/session retention window in days (`None` = retain indefinitely).
    pub raw_retention_days: Option<u32>,
    /// Creation timestamp in Unix milliseconds.
    pub created_at_ms: i64,
    /// Last update timestamp in Unix milliseconds.
    pub updated_at_ms: i64,
}

/// Control-plane metadata for a hashed Bearer API key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiKeyRecord {
    /// Immutable key UID.
    pub key_id: KeyId,
    /// Owning organization UID.
    pub org_id: OrgId,
    /// First 12 characters of the token for safe operator display.
    pub key_prefix: String,
    /// `SHA-256(token)` digest stored in `SQLite`.
    pub key_hash: [u8; 32],
    /// Granted authorization scope.
    pub scope: KeyScope,
    /// Bound site UID (`None` only for org-wide admin keys).
    pub site_id: Option<SiteId>,
    /// Human operator label.
    pub label: String,
    /// Creation timestamp in Unix milliseconds.
    pub created_at_ms: i64,
    /// Optional last-used timestamp in Unix milliseconds.
    pub last_used_ms: Option<i64>,
    /// Optional revocation timestamp in Unix milliseconds.
    pub revoked_at_ms: Option<i64>,
}
