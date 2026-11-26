#![forbid(unsafe_code)]
//! Wire DTOs for `POST /v1/events` and `POST /v1/events/batch`.

use std::collections::BTreeMap;

use rotameter_core::id::SiteRef;
use serde::{Deserialize, Serialize};

/// Maximum number of events permitted per `POST /v1/events/batch` request (`SPEC-API.md` §3.4).
pub const MAX_BATCH_EVENTS: usize = 500;

/// Single-event ingestion payload accepted by `POST /v1/events`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventPayload {
    /// Target site identifier (`site_...` or `slug`).
    pub site_id: SiteRef,
    /// Event name (defaults server-side to `"pageview"` when omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Full page URL or path.
    pub url: String,
    /// Optional referring URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referrer: Option<String>,
    /// Optional browser viewport width in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_width: Option<u16>,
    /// Optional custom event properties map.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub props: Option<BTreeMap<String, serde_json::Value>>,
}

/// Response returned by `POST /v1/events` (`202 Accepted`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventAcceptedResponse {
    /// `1` if enqueued, `0` if dropped by bot/`Sec-GPC` filtering.
    pub accepted: u64,
}

/// Single event entry within a `POST /v1/events/batch` payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchEventItem {
    /// Target site identifier (`site_...` or `slug`).
    pub site_id: SiteRef,
    /// Event name (defaults server-side to `"pageview"` when omitted).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    /// Full page URL or path.
    pub url: String,
    /// Optional referring URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referrer: Option<String>,
    /// Optional browser viewport width in pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub viewport_width: Option<u16>,
    /// Optional custom event properties map.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub props: Option<BTreeMap<String, serde_json::Value>>,
    /// Explicit historical event timestamp (`RFC 3339` UTC).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ts: Option<String>,
    /// Optional synthetic visitor hint hashed with the rotating salt for log imports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visitor_hint: Option<String>,
    /// Optional per-item `User-Agent` string for browser/OS/bot classification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
    /// Optional ISO-3166-1 alpha-2 country override for log imports.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// Optional ephemeral client IP hashed with the rotating salt and immediately discarded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
}

/// Server-to-server batch ingest request accepted by `POST /v1/events/batch`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchEventsRequest {
    /// Batch of up to `500` events.
    pub events: Vec<BatchEventItem>,
}

/// Response returned by `POST /v1/events/batch` (`202 Accepted`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchEventsResponse {
    /// Count of events accepted into the ingest pipeline.
    pub accepted: u64,
    /// Count of events rejected or filtered.
    pub rejected: u64,
}
