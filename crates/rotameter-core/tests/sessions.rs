#![forbid(unsafe_code)]
//! Integration tests for `30m` sliding sessions, midnight carryover, LRU eviction, and expiry.

use rotameter_core::id::SiteId;
use rotameter_core::privacy::{
    InMemorySaltProvider, MS_PER_DAY, SaltWindow, resolve_visitor_candidates,
};
use rotameter_core::session::{DEFAULT_SESSION_TIMEOUT_MS, SessionSnapshot, SessionTracker};
use rotameter_core::types::SaltRotation;

#[test]
fn midnight_rollover_preserves_session_bounce_and_duration() {
    let provider = InMemorySaltProvider::new(|w: SaltWindow| {
        let byte = u8::try_from(w.window_id.rem_euclid(251)).unwrap_or(1);
        [byte; 32]
    });
    let site = SiteId::from_uuidv7_parts(1_700_000_000_000, [3; 10]);
    let mut tracker = SessionTracker::with_defaults();

    let ts_2355 = 20_000 * MS_PER_DAY + (23 * 60 + 55) * 60_000;
    let ts_0005 = 20_001 * MS_PER_DAY + 5 * 60_000;

    let c1 = resolve_visitor_candidates(
        &provider,
        site,
        SaltRotation::Daily,
        ts_2355,
        "203.0.113.9",
        "Firefox/132.0",
    )
    .unwrap();
    let d1 = tracker.record_event(site, c1, ts_2355, true, None, || [0xa1; 16]);
    assert!(d1.is_new_session);
    assert_eq!(d1.bounce_delta, 1);
    assert_eq!(d1.seq_in_session, 1);

    let c2 = resolve_visitor_candidates(
        &provider,
        site,
        SaltRotation::Daily,
        ts_0005,
        "203.0.113.9",
        "Firefox/132.0",
    )
    .unwrap();
    assert_ne!(c2.current, c1.current);
    assert_eq!(c2.previous, Some(c1.current));

    let d2 = tracker.record_event(site, c2, ts_0005, true, None, || [0xb2; 16]);
    assert!(!d2.is_new_session);
    assert_eq!(d2.session_id, d1.session_id);
    assert_eq!(d2.visitor_id, d1.visitor_id);
    assert_eq!(d2.seq_in_session, 2);
    assert_eq!(d2.event_count, 2);
    assert_eq!(d2.bounce_delta, -1);
    assert_eq!(d2.duration_delta_ms, 600_000);
}

#[test]
fn session_expires_after_timeout_and_rehydrates_from_fallback_on_eviction() {
    let provider = InMemorySaltProvider::new(|_| [7u8; 32]);
    let site = SiteId::from_uuidv7_parts(1_700_000_000_000, [4; 10]);
    let mut tracker = SessionTracker::new(1, DEFAULT_SESSION_TIMEOUT_MS);

    let c_a = resolve_visitor_candidates(
        &provider,
        site,
        SaltRotation::Daily,
        1_000,
        "192.0.2.1",
        "Safari/18",
    )
    .unwrap();
    let d_a1 = tracker.record_event(site, c_a, 1_000, true, None, || [1; 16]);

    let c_b = resolve_visitor_candidates(
        &provider,
        site,
        SaltRotation::Daily,
        2_000,
        "192.0.2.2",
        "Safari/18",
    )
    .unwrap();
    tracker.record_event(site, c_b, 2_000, true, None, || [2; 16]);
    assert_eq!(tracker.active_len(), 1);

    let fallback = SessionSnapshot {
        session_id: d_a1.session_id,
        visitor_id: d_a1.visitor_id,
        started_at_ms: 1_000,
        last_event_ms: 1_000,
        event_count: 1,
        pageview_count: 1,
        last_seq: 1,
    };
    let d_a2 = tracker.record_event(site, c_a, 61_000, false, Some(fallback), || [3; 16]);
    assert!(!d_a2.is_new_session);
    assert_eq!(d_a2.session_id, [1; 16]);
    assert_eq!(d_a2.seq_in_session, 2);

    let d_a3 = tracker.record_event(
        site,
        c_a,
        61_000 + DEFAULT_SESSION_TIMEOUT_MS + 1,
        true,
        None,
        || [4; 16],
    );
    assert!(d_a3.is_new_session);
    assert_eq!(d_a3.session_id, [4; 16]);
}
