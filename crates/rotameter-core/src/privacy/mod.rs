#![forbid(unsafe_code)]
//! Privacy-preserving `HMAC-SHA256` visitor hashing and salt window rotation (`SPEC-PRIVACY.md`).

pub mod hash;
pub mod window;

pub use hash::{
    InMemorySaltProvider, SaltBytes, SaltProvider, SessionId, VisitorCandidates, VisitorId,
    compute_api_key_hash, compute_visitor_id, resolve_visitor_candidates,
};
pub use window::{
    GRACE_WINDOW_MS, MS_PER_DAY, MS_PER_HOUR, SaltWindow, WindowResolution, resolve_salt_window,
    utc_hour_floor_ms, utc_yyyymmdd,
};
