#![forbid(unsafe_code)]
//! Pure-Rust storage exchange snapshot and rollup row models (`RM-33`).

use serde::{Deserialize, Serialize};

use crate::id::SiteId;
use crate::privacy::SaltBytes;
use crate::types::{ApiKeyRecord, Org, SaltRotation, Site};

/// Persisted rotating `HMAC-SHA256` salt window row (`rotating_salts`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RotatingSaltRecord {
    /// Rotation cadence (`daily`, `weekly`, or `monthly`).
    pub rotation: SaltRotation,
    /// Monotonic period index.
    pub window_id: i64,
    /// 32-byte CSPRNG salt.
    pub salt: SaltBytes,
    /// Creation timestamp in Unix milliseconds.
    pub created_at_ms: i64,
}

/// Complete OLTP control-plane snapshot exported for portable backups and cross-store migration.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlPlaneSnapshot {
    /// Tenant organizations.
    pub orgs: Vec<Org>,
    /// Tracked sites.
    pub sites: Vec<Site>,
    /// Hashed API keys.
    pub api_keys: Vec<ApiKeyRecord>,
    /// Persisted active rotating salts.
    pub rotating_salts: Vec<RotatingSaltRecord>,
}

/// Materialized row from the `rollups_hourly` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollupHourlyRow {
    /// Target site UID.
    pub site_id: SiteId,
    /// UTC hour floor timestamp in milliseconds (`bucket_ts_ms`).
    pub bucket_ts_ms: i64,
    /// Total pageviews in this hour.
    pub pageviews: i64,
    /// Total events in this hour.
    pub events_total: i64,
    /// Single-event sessions attributed to this hour.
    pub bounced_visits: i64,
    /// Sessions started in this hour.
    pub visits_started: i64,
    /// Cumulative session duration in milliseconds.
    pub total_duration_ms: i64,
}

/// Materialized row from the `rollups_daily` table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollupDailyRow {
    /// Target site UID.
    pub site_id: SiteId,
    /// UTC calendar date as `YYYYMMDD` integer.
    pub day_utc: i32,
    /// Total pageviews on this UTC day.
    pub pageviews: i64,
    /// Total events on this UTC day.
    pub events_total: i64,
    /// Single-event sessions attributed to this UTC day.
    pub bounced_visits: i64,
    /// Sessions started on this UTC day.
    pub visits_started: i64,
    /// Cumulative session duration in milliseconds.
    pub total_duration_ms: i64,
    /// Distinct visitors seen on this UTC day.
    pub visitors: i64,
}

/// Materialized row from the `rollups_path_daily` table (with string `path` decoded).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollupPathDailyRow {
    /// Target site UID.
    pub site_id: SiteId,
    /// UTC calendar date as `YYYYMMDD` integer.
    pub day_utc: i32,
    /// Normalized page path (`dim_strings.value`).
    pub path: String,
    /// Pageviews for `path` on this UTC day.
    pub pageviews: i64,
    /// Total events for `path` on this UTC day.
    pub events_total: i64,
    /// Distinct sessions touching `path` on this UTC day.
    pub visits: i64,
    /// Distinct visitors touching `path` on this UTC day.
    pub visitors: i64,
    /// Bounced sessions entering on `path` on this UTC day.
    pub bounced_visits: i64,
    /// Cumulative duration of sessions touching `path` on this UTC day.
    pub total_duration_ms: i64,
}
