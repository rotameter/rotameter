#![forbid(unsafe_code)]
//! Bounded `30`-minute sliding session state machine (`SPEC-PRIVACY.md` §3).

pub mod lru;

use std::fmt;

use crate::id::SiteId;
use crate::privacy::{SessionId, VisitorCandidates, VisitorId};

use lru::BoundedLru;

/// Default sliding inactivity session timeout (`30 minutes` in milliseconds).
pub const DEFAULT_SESSION_TIMEOUT_MS: i64 = 1_800_000;
/// Default active session LRU cap (`100,000` entries, `<= 16 MB` RAM).
pub const DEFAULT_MAX_ACTIVE_SESSIONS: usize = 100_000;

/// Cached or database-hydrated state of an active visitor session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionSnapshot {
    /// Immutable `16`-byte session identifier.
    pub session_id: SessionId,
    /// Visitor hash under which this session was opened.
    pub visitor_id: VisitorId,
    /// Session start timestamp in Unix milliseconds.
    pub started_at_ms: i64,
    /// Most recent event timestamp in Unix milliseconds.
    pub last_event_ms: i64,
    /// Total events observed in this session (`>= 1`).
    pub event_count: u32,
    /// Total `pageview` events observed in this session.
    pub pageview_count: u32,
    /// Highest monotonic `seq_in_session` assigned so far.
    pub last_seq: u32,
}

/// Storage delta emitted when advancing or opening a session for one incoming event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionDelta {
    /// Site identifier owning the session.
    pub site_id: SiteId,
    /// Session identifier assigned to the event.
    pub session_id: SessionId,
    /// Visitor identifier tied to the session (preserved across grace carryover).
    pub visitor_id: VisitorId,
    /// Strictly monotonic per-session event sequence (`1, 2, 3...`).
    pub seq_in_session: u32,
    /// `true` when this event opened a new session (`visits_started += 1`).
    pub is_new_session: bool,
    /// Change to rollup `bounced_visits` (`+1` on hit 1, `-1` on hit 2, `0` thereafter).
    pub bounce_delta: i32,
    /// Increment to rollup `total_duration_ms` (`ts_ms - prev_last_event_ms`, clamped `>= 0`).
    pub duration_delta_ms: i64,
    /// Updated session `event_count`.
    pub event_count: u32,
    /// Updated session `pageview_count`.
    pub pageview_count: u32,
    /// Session start timestamp in Unix milliseconds.
    pub started_at_ms: i64,
    /// Updated session `last_event_ms`.
    pub last_event_ms: i64,
}

/// Single-writer bounded `30`-minute sliding session state machine.
pub struct SessionTracker {
    lru: BoundedLru<(SiteId, VisitorId), SessionSnapshot>,
    timeout_ms: i64,
}

impl SessionTracker {
    /// Creates a tracker bounded to `max_entries` active sessions and `timeout_ms` inactivity.
    pub fn new(max_entries: usize, timeout_ms: i64) -> Self {
        Self {
            lru: BoundedLru::new(max_entries),
            timeout_ms,
        }
    }

    /// Creates a tracker with the canonical `v1.0` defaults (`100,000` entries, `30m` timeout).
    pub fn with_defaults() -> Self {
        Self::new(DEFAULT_MAX_ACTIVE_SESSIONS, DEFAULT_SESSION_TIMEOUT_MS)
    }

    /// Returns the number of active session entries currently cached in memory.
    pub fn active_len(&self) -> usize {
        self.lru.len()
    }

    /// Records one incoming event and returns the resulting [`SessionDelta`].
    ///
    /// Lookup checks `candidates.current`, then `candidates.previous` (for `30m` salt-rotation
    /// carryover), then an optional storage `fallback` hydrated after LRU eviction, before
    /// minting a new session via `next_session_id`.
    pub fn record_event(
        &mut self,
        site_id: SiteId,
        candidates: VisitorCandidates,
        ts_ms: i64,
        is_pageview: bool,
        fallback: Option<SessionSnapshot>,
        next_session_id: impl FnOnce() -> SessionId,
    ) -> SessionDelta {
        if let Some(snap) = self.lru.get_mut(&(site_id, candidates.current))
            && ts_ms.saturating_sub(snap.last_event_ms) <= self.timeout_ms
        {
            return advance_snapshot(site_id, snap, ts_ms, is_pageview);
        }

        if let Some(prev_visitor) = candidates.previous
            && let Some(snap) = self.lru.get_mut(&(site_id, prev_visitor))
            && ts_ms.saturating_sub(snap.last_event_ms) <= self.timeout_ms
        {
            return advance_snapshot(site_id, snap, ts_ms, is_pageview);
        }

        if let Some(mut snap) = fallback
            && ts_ms.saturating_sub(snap.last_event_ms) <= self.timeout_ms
        {
            let delta = advance_snapshot(site_id, &mut snap, ts_ms, is_pageview);
            self.lru.insert((site_id, snap.visitor_id), snap);
            return delta;
        }

        let snap = SessionSnapshot {
            session_id: next_session_id(),
            visitor_id: candidates.current,
            started_at_ms: ts_ms,
            last_event_ms: ts_ms,
            event_count: 1,
            pageview_count: u32::from(is_pageview),
            last_seq: 1,
        };
        self.lru.insert((site_id, candidates.current), snap);

        SessionDelta {
            site_id,
            session_id: snap.session_id,
            visitor_id: snap.visitor_id,
            seq_in_session: 1,
            is_new_session: true,
            bounce_delta: 1,
            duration_delta_ms: 0,
            event_count: 1,
            pageview_count: snap.pageview_count,
            started_at_ms: ts_ms,
            last_event_ms: ts_ms,
        }
    }
}

impl fmt::Debug for SessionTracker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionTracker")
            .field("active_len", &self.lru.len())
            .field("capacity", &self.lru.capacity())
            .field("timeout_ms", &self.timeout_ms)
            .finish()
    }
}

fn advance_snapshot(
    site_id: SiteId,
    snap: &mut SessionSnapshot,
    ts_ms: i64,
    is_pageview: bool,
) -> SessionDelta {
    let duration_delta_ms = ts_ms.saturating_sub(snap.last_event_ms);
    snap.last_seq = snap.last_seq.saturating_add(1);
    snap.event_count = snap.event_count.saturating_add(1);
    snap.pageview_count = snap.pageview_count.saturating_add(u32::from(is_pageview));
    snap.last_event_ms = snap.last_event_ms.max(ts_ms);

    let bounce_delta = if snap.event_count == 2 { -1 } else { 0 };

    SessionDelta {
        site_id,
        session_id: snap.session_id,
        visitor_id: snap.visitor_id,
        seq_in_session: snap.last_seq,
        is_new_session: false,
        bounce_delta,
        duration_delta_ms,
        event_count: snap.event_count,
        pageview_count: snap.pageview_count,
        started_at_ms: snap.started_at_ms,
        last_event_ms: snap.last_event_ms,
    }
}
