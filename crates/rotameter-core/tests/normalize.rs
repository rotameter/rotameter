#![forbid(unsafe_code)]
//! Integration tests for URL, UTM, referrer, and channel normalization.

use rotameter_core::error::ErrorCode;
use rotameter_core::normalize::{normalize_referrer, normalize_url};
use rotameter_core::types::{Channel, MAX_PATH_BYTES, UtmParams};

#[test]
fn normalizes_url_strips_fragments_and_sorts_allowed_query_keys() {
    let allowed_keys = vec!["range".to_string(), "tab".to_string()];
    let allowed_origins = vec!["https://colinwolff.dev".to_string()];

    let normalized = normalize_url(
        "https://ColinWolff.dev//projects/rotameter/?z=1&tab=geo&utm_source=HN&range=7d&utm_medium=social#top",
        &allowed_keys,
        &allowed_origins,
    )
    .unwrap();

    assert_eq!(normalized.host, "colinwolff.dev");
    assert_eq!(normalized.path, "/projects/rotameter?range=7d&tab=geo");
    assert_eq!(normalized.utm.source.as_deref(), Some("HN"));
    assert_eq!(normalized.utm.medium.as_deref(), Some("social"));
}

#[test]
fn enforces_http_schemes_host_allowlist_and_utf8_path_cap() {
    assert_eq!(
        normalize_url("javascript:alert(1)", &[], &[])
            .unwrap_err()
            .code(),
        ErrorCode::InvalidUrl
    );
    assert_eq!(
        normalize_url(
            "https://evil.example/path",
            &[],
            &["colinwolff.dev".to_string()]
        )
        .unwrap_err()
        .code(),
        ErrorCode::HostMismatch
    );

    let long_url = format!("https://colinwolff.dev/{}", "🦀".repeat(200));
    let capped = normalize_url(&long_url, &[], &[]).unwrap();
    assert!(capped.path.len() <= MAX_PATH_BYTES);
    assert_eq!(
        normalize_url("https://colinwolff.dev", &[], &[])
            .unwrap()
            .path,
        "/"
    );
}

#[test]
fn normalizes_referrers_aliases_self_referrals_and_channels() {
    let no_utm = UtmParams::default();
    let origins = vec!["https://colinwolff.dev".to_string()];

    let self_ref = normalize_referrer(
        Some("https://www.colinwolff.dev/about?x=1"),
        "colinwolff.dev",
        &origins,
        &no_utm,
    );
    assert_eq!(self_ref.source, "(direct)");
    assert_eq!(self_ref.host, None);
    assert_eq!(self_ref.channel, Channel::Direct);

    let google = normalize_referrer(
        Some("https://www.google.co.uk/search?q=rotameter"),
        "colinwolff.dev",
        &origins,
        &no_utm,
    );
    assert_eq!(google.source, "google.com");
    assert_eq!(google.channel, Channel::OrganicSearch);
    assert_eq!(
        google.referrer_url.as_deref(),
        Some("https://www.google.co.uk/search")
    );

    let social = normalize_referrer(
        Some("https://t.co/abc123?s=1"),
        "colinwolff.dev",
        &origins,
        &no_utm,
    );
    assert_eq!(social.source, "x.com");
    assert_eq!(social.channel, Channel::OrganicSocial);

    let paid_utm = UtmParams {
        medium: Some("cpc".to_string()),
        ..Default::default()
    };
    let paid = normalize_referrer(
        Some("https://www.google.com/"),
        "colinwolff.dev",
        &origins,
        &paid_utm,
    );
    assert_eq!(paid.channel, Channel::PaidCampaign);
}
