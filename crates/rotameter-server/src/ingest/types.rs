#![forbid(unsafe_code)]
//! Queued event representation and session fallback lookup helper for batch processing.

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::VisitorCandidates;
use rotameter_core::session::SessionSnapshot;
use rotameter_core::types::{Channel, DeviceType, EventKind, EventProps, UtmParams, ViewportClass};

use crate::state::ServerStore;

pub(crate) struct QueuedEvent {
    pub(crate) site_id: SiteId,
    pub(crate) ts_ms: i64,
    pub(crate) candidates: VisitorCandidates,
    pub(crate) event_kind: EventKind,
    pub(crate) path: String,
    pub(crate) props: EventProps,
    pub(crate) ref_host: String,
    pub(crate) channel: Channel,
    pub(crate) utm: UtmParams,
    pub(crate) country: String,
    pub(crate) region: String,
    pub(crate) browser: String,
    pub(crate) os: String,
    pub(crate) device_type: DeviceType,
    pub(crate) viewport_class: ViewportClass,
}

pub(crate) fn lookup_fallback(
    store: &dyn ServerStore,
    site_id: SiteId,
    candidates: VisitorCandidates,
    min_last_ms: i64,
) -> Result<Option<SessionSnapshot>, CoreError> {
    if let Some(found) = store.lookup_active_session(site_id, candidates.current, min_last_ms)? {
        return Ok(Some(found));
    }
    if let Some(prev) = candidates.previous {
        return store.lookup_active_session(site_id, prev, min_last_ms);
    }
    Ok(None)
}
