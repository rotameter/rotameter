#![forbid(unsafe_code)]
//! Diurnal curve weights and weighted dimension catalogs for `rotameter seed`.

use rotameter_core::types::{Channel, DeviceType, ViewportClass};

/// Bimodal UTC diurnal curve weights per hour (`0..24`).
pub(crate) const DIURNAL_WEIGHTS: [u64; 24] = [
    32, 24, 20, 18, 22, 28, 40, 58, 76, 92, 102, 106, 98, 104, 118, 130, 134, 128, 114, 98, 84, 70,
    54, 40,
];

pub(crate) fn day_multiplier_permille(day_idx: u32, total_days: u32) -> u64 {
    let week_pattern = [920_u64, 1060, 1140, 1120, 1050, 820, 760];
    let base = week_pattern[usize::try_from(day_idx % 7).unwrap_or(0)];
    let growth = u64::from(day_idx).saturating_mul(180) / u64::from(total_days.max(1));
    let launch_spike = if total_days >= 7 && day_idx == (total_days * 2) / 3 {
        1_150
    } else {
        0
    };
    base + growth + launch_spike
}

pub(crate) const PATHS: &[(u64, &str)] = &[
    (26, "/"),
    (16, "/projects/rotameter"),
    (11, "/blog/sqlite-wal-analytics"),
    (9, "/blog/cloudflare-workers-proxy"),
    (8, "/docs/getting-started"),
    (7, "/docs/privacy-model"),
    (5, "/docs/api-reference"),
    (4, "/blog/edge-ssr-analytics"),
    (3, "/about"),
    (3, "/changelog"),
    (2, "/docs/litestream-r2"),
    (2, "/docs/astro-cloudflare"),
    (1, "/pricing"),
    (1, "/uses"),
    (1, "/rss.xml"),
    (1, "/404"),
];

pub(crate) type SourceSpec = (
    &'static str,
    Channel,
    Option<&'static str>,
    Option<&'static str>,
    Option<&'static str>,
);

pub(crate) const SOURCES: &[(u64, SourceSpec)] = &[
    (30, ("(direct)", Channel::Direct, None, None, None)),
    (
        16,
        ("www.google.com", Channel::OrganicSearch, None, None, None),
    ),
    (
        14,
        ("news.ycombinator.com", Channel::Referral, None, None, None),
    ),
    (
        10,
        (
            "github.com",
            Channel::Referral,
            Some("github"),
            Some("readme"),
            Some("oss"),
        ),
    ),
    (
        7,
        ("duckduckgo.com", Channel::OrganicSearch, None, None, None),
    ),
    (6, ("bsky.app", Channel::OrganicSocial, None, None, None)),
    (5, ("lobste.rs", Channel::Referral, None, None, None)),
    (4, ("kagi.com", Channel::OrganicSearch, None, None, None)),
    (3, ("t.co", Channel::OrganicSocial, None, None, None)),
    (
        3,
        (
            "newsletter.example.com",
            Channel::Email,
            Some("newsletter"),
            Some("email"),
            Some("issue-14"),
        ),
    ),
    (
        2,
        ("www.reddit.com", Channel::OrganicSocial, None, None, None),
    ),
];

pub(crate) const GEOS: &[(u64, (&str, &str))] = &[
    (38, ("US", "CA")),
    (14, ("US", "NY")),
    (10, ("DE", "BE")),
    (9, ("GB", "ENG")),
    (7, ("CA", "ON")),
    (6, ("FR", "IDF")),
    (5, ("NL", "NH")),
    (4, ("JP", "13")),
    (4, ("AU", "NSW")),
    (3, ("SE", "AB")),
];

pub(crate) type ClientSpec = (&'static str, &'static str, DeviceType, ViewportClass);

pub(crate) const CLIENTS: &[(u64, ClientSpec)] = &[
    (
        34,
        (
            "Chrome 131",
            "macOS 15",
            DeviceType::Desktop,
            ViewportClass::Xl,
        ),
    ),
    (
        18,
        (
            "Firefox 132",
            "Linux",
            DeviceType::Desktop,
            ViewportClass::Lg,
        ),
    ),
    (
        14,
        ("Safari 18", "iOS 18", DeviceType::Mobile, ViewportClass::Xs),
    ),
    (
        12,
        (
            "Chrome 131",
            "Windows 11",
            DeviceType::Desktop,
            ViewportClass::Xl,
        ),
    ),
    (
        10,
        (
            "Chrome 131",
            "Android 15",
            DeviceType::Mobile,
            ViewportClass::Sm,
        ),
    ),
    (
        7,
        (
            "Safari 18",
            "macOS 15",
            DeviceType::Desktop,
            ViewportClass::Lg,
        ),
    ),
    (
        5,
        (
            "Safari 18",
            "iPadOS 18",
            DeviceType::Tablet,
            ViewportClass::Md,
        ),
    ),
];
