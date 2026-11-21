#![forbid(unsafe_code)]
//! Batched ingestion write models shared between server ring-buffer flusher and storage adapters.

use serde::{Deserialize, Serialize};

use crate::id::SiteId;
use crate::privacy::{SessionId, VisitorId};
use crate::types::{Channel, DeviceType, EventKind, EventProps, UtmParams, ViewportClass};

/// Single normalized event ready to be interned and written to storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestEvent {
    /// Target site UID.
    pub site_id: SiteId,
    /// Event timestamp in Unix milliseconds.
    pub ts_ms: i64,
    /// Assigned session ID.
    pub session_id: SessionId,
    /// Monotonic sequence index within the session.
    pub seq_in_session: u32,
    /// Pseudonymous `HMAC-SHA256` visitor ID.
    pub visitor_id: VisitorId,
    /// Normalized event kind.
    pub event_kind: EventKind,
    /// Normalized path+query (`<= 512` bytes).
    pub path: String,
    /// Scalar custom properties.
    pub props: EventProps,
}

/// Full session upsert emitted alongside one or more events in a [`WriteBatch`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionUpsert {
    /// Target site UID.
    pub site_id: SiteId,
    /// Session ID.
    pub session_id: SessionId,
    /// Pseudonymous visitor ID.
    pub visitor_id: VisitorId,
    /// Session start timestamp in Unix milliseconds.
    pub started_at_ms: i64,
    /// Latest event timestamp in Unix milliseconds.
    pub last_event_ms: i64,
    /// Total events in this session.
    pub event_count: u32,
    /// Total pageviews in this session.
    pub pageview_count: u32,
    /// Entry page path (`<= 512` bytes).
    pub entry_path: String,
    /// Latest exit page path (`<= 512` bytes).
    pub exit_path: String,
    /// Canonical referrer source (`"(direct)"` or host).
    pub ref_host: String,
    /// Acquisition channel.
    pub channel: Channel,
    /// Extracted UTM parameters.
    pub utm: UtmParams,
    /// Coarse country code (`(unknown)` when absent).
    pub country: String,
    /// Coarse subdivision/region code (`(unknown)` when absent).
    pub region: String,
    /// Coarse browser + major version.
    pub browser: String,
    /// Coarse OS + major version.
    pub os: String,
    /// Coarse device hardware form factor.
    pub device_type: DeviceType,
    /// Tailwind-aligned viewport bucket.
    pub viewport_class: ViewportClass,
}

/// Pre-aggregated hourly and daily counter delta accumulated for one site bucket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RollupDelta {
    /// Target site UID.
    pub site_id: SiteId,
    /// UTC hour floor timestamp in milliseconds (`rollups_hourly.bucket_ts_ms`).
    pub hour_bucket_ms: i64,
    /// UTC `YYYYMMDD` integer (`rollups_daily.day_utc`).
    pub day_utc: i32,
    /// Net increment to `pageviews`.
    pub pageviews: i64,
    /// Net increment to `events_total`.
    pub events_total: i64,
    /// Net delta to `bounced_visits` (can be negative when a session receives its 2nd event).
    pub bounced_visits: i64,
    /// Net increment to `visits_started`.
    pub visits_started: i64,
    /// Net increment to `total_duration_ms`.
    pub total_duration_ms: i64,
}

/// Atomic batch persisted in one `BEGIN IMMEDIATE ... COMMIT` storage transaction.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WriteBatch {
    /// Raw ordered events to append.
    pub events: Vec<IngestEvent>,
    /// Session rows to insert or update.
    pub sessions: Vec<SessionUpsert>,
    /// Hourly/daily counter deltas to fold.
    pub rollups: Vec<RollupDelta>,
}
