#![forbid(unsafe_code)]
//! Daily per-site dimension cardinality protection (`SPEC-PRIVACY.md` §6.3).

use std::collections::{HashMap, HashSet};

use crate::id::SiteId;
use crate::privacy::MS_PER_DAY;
use crate::types::DimensionKind;

/// Canonical sentinel value mapped when a site exceeds its daily dimension cap.
pub const OVERFLOW_BUCKET: &str = "(other)";

/// Daily per-site dimension uniqueness caps before collapsing unseen values into `(other)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CardinalityLimits {
    /// Maximum novel normalized URL paths per site per UTC day (`10,000`).
    pub paths_per_day: usize,
    /// Maximum novel referrer hosts/URLs per site per UTC day (`5,000`).
    pub referrers_per_day: usize,
    /// Maximum novel values for all other dimensions per site per UTC day (`2,000`).
    pub custom_values_per_day: usize,
}

impl Default for CardinalityLimits {
    fn default() -> Self {
        Self {
            paths_per_day: 10_000,
            referrers_per_day: 5_000,
            custom_values_per_day: 2_000,
        }
    }
}

impl CardinalityLimits {
    /// Returns the configured daily cap for `kind`.
    pub const fn cap_for(self, kind: DimensionKind) -> usize {
        match kind {
            DimensionKind::Path => self.paths_per_day,
            DimensionKind::RefHost | DimensionKind::RefUrl => self.referrers_per_day,
            DimensionKind::EventName
            | DimensionKind::Utm
            | DimensionKind::UaBrowser
            | DimensionKind::UaOs
            | DimensionKind::GeoCountry
            | DimensionKind::GeoRegion
            | DimensionKind::Channel => self.custom_values_per_day,
        }
    }
}

/// Result of checking a dimension string against the site's daily cardinality budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardinalityOutcome<'a> {
    /// Value is within budget (or already seen today) and kept verbatim.
    Accepted(&'a str),
    /// Daily cap reached for novel values; caller must record [`OVERFLOW_BUCKET`].
    Overflowed,
}

impl<'a> CardinalityOutcome<'a> {
    /// Returns the string that should be interned (`value` or `"(other)"`).
    pub const fn as_str(self) -> &'a str {
        match self {
            Self::Accepted(value) => value,
            Self::Overflowed => OVERFLOW_BUCKET,
        }
    }

    /// Returns `true` when the value was collapsed into `(other)`.
    pub const fn is_overflowed(self) -> bool {
        matches!(self, Self::Overflowed)
    }
}

#[derive(Debug, Clone)]
struct DailyBucket {
    day_index: i64,
    seen: HashSet<String>,
}

/// Tracks per-site daily unique dimension values and maps excess novel values to `(other)`.
#[derive(Debug, Clone)]
pub struct CardinalityGuard {
    limits: CardinalityLimits,
    buckets: HashMap<(SiteId, DimensionKind), DailyBucket>,
}

impl CardinalityGuard {
    /// Creates a cardinality guard using `limits`.
    pub fn new(limits: CardinalityLimits) -> Self {
        Self {
            limits,
            buckets: HashMap::new(),
        }
    }

    /// Creates a cardinality guard with the `v1.0` default daily caps.
    pub fn with_defaults() -> Self {
        Self::new(CardinalityLimits::default())
    }

    /// Records `value` for `(site_id, kind)` on the UTC day of `ts_ms`, returning whether it fits.
    ///
    /// Automatically resets a bucket when `ts_ms` advances to a different UTC day.
    pub fn check_or_record<'a>(
        &mut self,
        site_id: SiteId,
        kind: DimensionKind,
        ts_ms: i64,
        value: &'a str,
    ) -> CardinalityOutcome<'a> {
        if value == OVERFLOW_BUCKET {
            return CardinalityOutcome::Overflowed;
        }
        let day_index = ts_ms.div_euclid(MS_PER_DAY);
        let cap = self.limits.cap_for(kind);
        let bucket = self
            .buckets
            .entry((site_id, kind))
            .or_insert_with(|| DailyBucket {
                day_index,
                seen: HashSet::new(),
            });

        if bucket.day_index != day_index {
            bucket.day_index = day_index;
            bucket.seen.clear();
        }
        if bucket.seen.contains(value) {
            return CardinalityOutcome::Accepted(value);
        }
        if bucket.seen.len() < cap {
            bucket.seen.insert(value.to_string());
            CardinalityOutcome::Accepted(value)
        } else {
            CardinalityOutcome::Overflowed
        }
    }
}
