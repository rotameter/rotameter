#![forbid(unsafe_code)]
//! Selectable metrics and dimensions for `v1.0` analytics queries.

use serde::{Deserialize, Serialize};

/// Selectable `v1.0` analytics metrics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Metric {
    /// Distinct pseudonymous visitors in the query window.
    Visitors,
    /// Total `pageview` events in the query window.
    Pageviews,
    /// Total 30-minute sliding sessions started in the query window.
    Visits,
    /// `pageviews / max(visits, 1)`.
    ViewsPerVisit,
    /// Ratio of single-event sessions (`[0.0, 1.0]`).
    BounceRate,
    /// Arithmetic mean session duration in milliseconds.
    AvgVisitDurationMs,
}

/// Group-by and filter dimensions supported in `v1.0`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Dimension {
    /// Normalized URL path.
    Path,
    /// Entry page path of the session.
    EntryPage,
    /// Exit page path of the session.
    ExitPage,
    /// Normalized referrer source / host.
    Source,
    /// Full capped referrer URL.
    Referrer,
    /// Acquisition channel label.
    Channel,
    /// Country code.
    Country,
    /// Region / subdivision code.
    Region,
    /// Coarse browser + major version.
    Browser,
    /// Coarse OS + major version.
    Os,
    /// Coarse hardware device type.
    Device,
    /// Viewport breakpoint bucket.
    Viewport,
    /// Event name (`pageview`, custom, etc.).
    EventName,
    /// `utm_source`.
    UtmSource,
    /// `utm_medium`.
    UtmMedium,
    /// `utm_campaign`.
    UtmCampaign,
}
