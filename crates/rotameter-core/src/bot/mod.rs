#![forbid(unsafe_code)]
//! Aho-Corasick bot, crawler, and prefetch traffic filter (`SPEC-PRIVACY.md` §4).

mod patterns;

use std::sync::LazyLock;

use aho_corasick::{AhoCorasick, AhoCorasickBuilder};

use crate::normalize::truncate_utf8;
use crate::types::MAX_USER_AGENT_BYTES;

use patterns::BOT_PATTERNS;

static BOT_MATCHER: LazyLock<Option<AhoCorasick>> = LazyLock::new(|| {
    AhoCorasickBuilder::new()
        .ascii_case_insensitive(true)
        .build(BOT_PATTERNS)
        .ok()
});

/// Prefetch / prerender HTTP headers inspected before counting a hit.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct PrefetchHeaders<'a> {
    /// `Sec-Purpose` header value.
    pub sec_purpose: Option<&'a str>,
    /// `Purpose` header value.
    pub purpose: Option<&'a str>,
    /// Legacy `X-Moz` header value.
    pub x_moz: Option<&'a str>,
}

/// Reason an incoming request was classified as non-human / automated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BotDropReason {
    /// Request carried a browser prefetch or prerender hint.
    Prefetch,
    /// Browser ingest call omitted a `User-Agent` header.
    MissingUserAgent,
    /// `User-Agent` matched the compiled-in crawler/automation signature set.
    KnownBot,
}

/// Outcome of bot and prefetch traffic inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BotFilterDecision {
    /// Treat hit as human traffic (or explicitly permitted synthetic telemetry).
    Allow,
    /// Drop hit and increment diagnostic bot counter.
    Drop(BotDropReason),
}

/// Classifies an incoming request as human (`Allow`) or synthetic (`Drop`).
///
/// Prefetch/prerender headers are always dropped unless `allow_automated` is `true`. Empty or
/// missing `User-Agent` strings are dropped when `require_user_agent` is `true` (browser
/// collection paths).
pub fn classify_traffic(
    user_agent: Option<&str>,
    headers: PrefetchHeaders<'_>,
    require_user_agent: bool,
    allow_automated: bool,
) -> BotFilterDecision {
    if !allow_automated && is_prefetch(headers) {
        return BotFilterDecision::Drop(BotDropReason::Prefetch);
    }
    if allow_automated {
        return BotFilterDecision::Allow;
    }

    let trimmed_ua = user_agent.map_or("", str::trim);
    if trimmed_ua.is_empty() {
        return if require_user_agent {
            BotFilterDecision::Drop(BotDropReason::MissingUserAgent)
        } else {
            BotFilterDecision::Allow
        };
    }

    let capped = truncate_utf8(trimmed_ua, MAX_USER_AGENT_BYTES);
    if let Some(matcher) = BOT_MATCHER.as_ref()
        && matcher.is_match(capped)
    {
        return BotFilterDecision::Drop(BotDropReason::KnownBot);
    }
    BotFilterDecision::Allow
}

fn is_prefetch(headers: PrefetchHeaders<'_>) -> bool {
    [headers.sec_purpose, headers.purpose, headers.x_moz]
        .into_iter()
        .flatten()
        .any(|value| {
            let lower = value.to_ascii_lowercase();
            lower.contains("prefetch") || lower.contains("prerender")
        })
}
