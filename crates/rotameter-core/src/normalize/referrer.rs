#![forbid(unsafe_code)]
//! Referrer host/alias normalization, self-referral stripping, and channel classification.

use url::Url;

use crate::normalize::truncate_utf8;
use crate::normalize::url::{host_matches_origins, normalize_path, strip_www};
use crate::types::{Channel, MAX_PATH_BYTES, UtmParams};

/// Normalized referrer attribution produced per event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedReferrer {
    /// Canonical source label (e.g., `"google.com"`, `"x.com"`, or `"(direct)"`).
    pub source: String,
    /// Canonical external referrer host, or `None` for direct/self-referred visits.
    pub host: Option<String>,
    /// Query-stripped external referrer URL (`<= 512` bytes), or `None` when direct.
    pub referrer_url: Option<String>,
    /// Assigned acquisition channel.
    pub channel: Channel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AliasKind {
    Search,
    Social,
    Email,
}

/// Normalizes an optional HTTP referrer against the site host and extracted UTM parameters.
pub fn normalize_referrer(
    raw_referrer: Option<&str>,
    site_host: &str,
    allowed_origins: &[String],
    utm: &UtmParams,
) -> NormalizedReferrer {
    let external =
        raw_referrer.and_then(|raw| parse_external_referrer(raw, site_host, allowed_origins));

    if let Some((canonical_host, kind, referrer_url)) = external {
        let channel = classify_channel(utm, kind, true);
        return NormalizedReferrer {
            source: canonical_host.clone(),
            host: Some(canonical_host),
            referrer_url: Some(referrer_url),
            channel,
        };
    }

    NormalizedReferrer {
        source: "(direct)".to_string(),
        host: None,
        referrer_url: None,
        channel: classify_channel(utm, None, false),
    }
}

fn parse_external_referrer(
    raw: &str,
    site_host: &str,
    allowed_origins: &[String],
) -> Option<(String, Option<AliasKind>, String)> {
    let parsed = Url::parse(raw.trim()).ok()?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return None;
    }
    let raw_host = parsed.host_str()?.to_ascii_lowercase();
    if raw_host.is_empty() {
        return None;
    }
    let site_host_lower = site_host.to_ascii_lowercase();
    if strip_www(&raw_host) == strip_www(&site_host_lower)
        || (!allowed_origins.is_empty() && host_matches_origins(&raw_host, allowed_origins))
    {
        return None;
    }

    let (canonical_host, kind) = canonicalize_host(&raw_host);
    let clean_path = normalize_path(parsed.path());
    let full_url = format!("{}://{raw_host}{clean_path}", parsed.scheme());
    let capped_url = truncate_utf8(&full_url, MAX_PATH_BYTES).to_string();
    Some((canonical_host, kind, capped_url))
}

fn canonicalize_host(raw_host: &str) -> (String, Option<AliasKind>) {
    let bare = raw_host
        .strip_prefix("www.")
        .or_else(|| raw_host.strip_prefix("m."))
        .or_else(|| raw_host.strip_prefix("mobile."))
        .unwrap_or(raw_host);

    if let Some((alias, kind)) = lookup_alias(bare) {
        return (alias.to_string(), Some(kind));
    }
    (bare.to_string(), None)
}

fn lookup_alias(bare: &str) -> Option<(&'static str, AliasKind)> {
    if bare == "google.com" || bare.starts_with("google.") {
        return Some(("google.com", AliasKind::Search));
    }
    if bare == "yandex.com" || bare.starts_with("yandex.") {
        return Some(("yandex.com", AliasKind::Search));
    }
    match bare {
        "bing.com" | "cn.bing.com" => Some(("bing.com", AliasKind::Search)),
        "duckduckgo.com" => Some(("duckduckgo.com", AliasKind::Search)),
        "search.brave.com" | "brave.com" => Some(("brave.com", AliasKind::Search)),
        "kagi.com" => Some(("kagi.com", AliasKind::Search)),
        "search.yahoo.com" | "r.search.yahoo.com" | "yahoo.com" => {
            Some(("yahoo.com", AliasKind::Search))
        }
        "ecosia.org" => Some(("ecosia.org", AliasKind::Search)),
        "baidu.com" => Some(("baidu.com", AliasKind::Search)),
        "startpage.com" => Some(("startpage.com", AliasKind::Search)),
        "qwant.com" => Some(("qwant.com", AliasKind::Search)),
        "t.co" | "twitter.com" | "x.com" => Some(("x.com", AliasKind::Social)),
        "l.instagram.com" | "instagram.com" => Some(("instagram.com", AliasKind::Social)),
        "l.facebook.com" | "lm.facebook.com" | "facebook.com" | "fb.com" => {
            Some(("facebook.com", AliasKind::Social))
        }
        "lnkd.in" | "linkedin.com" => Some(("linkedin.com", AliasKind::Social)),
        "out.reddit.com" | "old.reddit.com" | "new.reddit.com" | "reddit.com" => {
            Some(("reddit.com", AliasKind::Social))
        }
        "news.ycombinator.com" => Some(("news.ycombinator.com", AliasKind::Social)),
        "lobste.rs" => Some(("lobste.rs", AliasKind::Social)),
        "bsky.app" => Some(("bsky.app", AliasKind::Social)),
        "threads.net" => Some(("threads.net", AliasKind::Social)),
        "youtu.be" | "youtube.com" => Some(("youtube.com", AliasKind::Social)),
        "tiktok.com" => Some(("tiktok.com", AliasKind::Social)),
        "pinterest.com" | "pin.it" => Some(("pinterest.com", AliasKind::Social)),
        "mail.google.com" | "gmail.com" => Some(("gmail.com", AliasKind::Email)),
        "outlook.live.com" | "outlook.office.com" => Some(("outlook.com", AliasKind::Email)),
        "mail.yahoo.com" => Some(("yahoo-mail.com", AliasKind::Email)),
        "mail.proton.me" => Some(("proton.me", AliasKind::Email)),
        _ => None,
    }
}

fn classify_channel(utm: &UtmParams, kind: Option<AliasKind>, has_external_ref: bool) -> Channel {
    if let Some(medium) = utm.medium.as_deref() {
        let m = medium.to_ascii_lowercase();
        if matches!(
            m.as_str(),
            "cpc"
                | "ppc"
                | "paid"
                | "paid_social"
                | "paidsocial"
                | "paid-search"
                | "paid_search"
                | "display"
                | "cpm"
                | "banner"
                | "retargeting"
        ) {
            return Channel::PaidCampaign;
        }
        if matches!(m.as_str(), "email" | "newsletter" | "e-mail") {
            return Channel::Email;
        }
        if matches!(m.as_str(), "social" | "social-media" | "social_media") {
            return Channel::OrganicSocial;
        }
    }

    match kind {
        Some(AliasKind::Search) => Channel::OrganicSearch,
        Some(AliasKind::Social) => Channel::OrganicSocial,
        Some(AliasKind::Email) => Channel::Email,
        None if has_external_ref => Channel::Referral,
        None => Channel::Direct,
    }
}
