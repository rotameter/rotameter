#![forbid(unsafe_code)]
//! Wire DTOs for `GET /v1/sites/{site}/realtime`.

use rotameter_core::id::SiteId;
use rotameter_core::storage::RealtimePage;
use serde::{Deserialize, Serialize};

/// Single 1-minute pulse point returned by `GET /v1/sites/{site}/realtime`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealtimePulsePoint {
    /// Minute floor timestamp (`RFC 3339` UTC).
    pub minute: String,
    /// Events recorded during that minute.
    pub events: u64,
}

/// Typed response returned by `GET /v1/sites/{site}/realtime`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RealtimeResponse {
    /// Immutable target site UID.
    pub site_id: SiteId,
    /// Org-unique site slug.
    pub slug: String,
    /// Distinct visitors active over the trailing 5 minutes.
    pub active_visitors: u64,
    /// Top active pages over the trailing 5 minutes.
    pub active_pages: Vec<RealtimePage>,
    /// Minute-by-minute pulse over the trailing 30 minutes.
    pub pulse: Vec<RealtimePulsePoint>,
}
