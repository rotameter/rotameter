#![forbid(unsafe_code)]
//! Deterministic URL, UTM, referrer, and UTF-8 string normalization.

pub mod referrer;
pub mod url;

pub use referrer::{NormalizedReferrer, normalize_referrer};
pub use url::{NormalizedUrl, normalize_url};

/// Truncates `input` cleanly at or below `max_bytes` on a valid UTF-8 character boundary.
pub fn truncate_utf8(input: &str, max_bytes: usize) -> &str {
    if input.len() <= max_bytes {
        return input;
    }
    let mut end = max_bytes;
    while end > 0 && !input.is_char_boundary(end) {
        end -= 1;
    }
    &input[..end]
}
