#![forbid(unsafe_code)]
//! HTTP(S) URL normalization, `utm_*` extraction, and query-key allowlisting.

use url::{Url, form_urlencoded};

use crate::error::CoreError;
use crate::normalize::truncate_utf8;
use crate::types::{MAX_PATH_BYTES, MAX_UTM_BYTES, UtmParams};

/// Normalized URL components produced from a raw event payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedUrl {
    /// Lowercased hostname extracted from the URL.
    pub host: String,
    /// Normalized path plus sorted allowlisted query parameters, capped at `512` bytes.
    pub path: String,
    /// Extracted `utm_*` campaign parameters.
    pub utm: UtmParams,
}

/// Validates and normalizes an incoming page or event URL.
///
/// Enforces `http`/`https`, validates `host` against `allowed_origins` when configured, strips
/// `#fragment`, extracts `utm_*`, keeps only keys listed in `allowed_query_keys` sorted
/// deterministically, normalizes slashes, and caps the final path at `512` bytes.
///
/// # Errors
///
/// Returns [`CoreError::InvalidUrl`] on malformed/non-HTTP(S) URLs or [`CoreError::HostMismatch`]
/// when `allowed_origins` is non-empty and excludes the parsed host.
pub fn normalize_url(
    raw_url: &str,
    allowed_query_keys: &[String],
    allowed_origins: &[String],
) -> Result<NormalizedUrl, CoreError> {
    let parsed = Url::parse(raw_url.trim())
        .map_err(|err| CoreError::InvalidUrl(format!("cannot parse URL: {err}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(CoreError::InvalidUrl(format!(
            "unsupported URL scheme `{}`",
            parsed.scheme()
        )));
    }
    let host = parsed
        .host_str()
        .map(str::to_ascii_lowercase)
        .filter(|h| !h.is_empty())
        .ok_or_else(|| CoreError::InvalidUrl("URL is missing a host".to_string()))?;

    if !allowed_origins.is_empty() && !host_matches_origins(&host, allowed_origins) {
        return Err(CoreError::HostMismatch(host));
    }

    let mut utm = UtmParams::default();
    let mut kept: Vec<(String, String)> = Vec::new();
    for (raw_key, raw_val) in parsed.query_pairs() {
        let key_lower = raw_key.to_ascii_lowercase();
        if assign_utm(&mut utm, &key_lower, raw_val.as_ref()) {
            continue;
        }
        if allowed_query_keys
            .iter()
            .any(|allowed| allowed == raw_key.as_ref())
        {
            kept.push((raw_key.into_owned(), raw_val.into_owned()));
        }
    }
    kept.sort();

    let mut path = normalize_path(parsed.path());
    if !kept.is_empty() {
        let encoded = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(kept)
            .finish();
        path.push('?');
        path.push_str(&encoded);
    }
    let capped_path = truncate_utf8(&path, MAX_PATH_BYTES).to_string();

    Ok(NormalizedUrl {
        host,
        path: capped_path,
        utm,
    })
}

pub(crate) fn host_matches_origins(host: &str, allowed_origins: &[String]) -> bool {
    allowed_origins
        .iter()
        .filter_map(|origin| extract_origin_host(origin))
        .any(|allowed| strip_www(&allowed) == strip_www(host))
}

pub(crate) fn extract_origin_host(origin: &str) -> Option<String> {
    let trimmed = origin.trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.contains("://") {
        Url::parse(trimmed)
            .ok()?
            .host_str()
            .map(str::to_ascii_lowercase)
    } else {
        let without_path = trimmed.split('/').next().unwrap_or(trimmed);
        let host = without_path.split(':').next().unwrap_or(without_path);
        (!host.is_empty()).then(|| host.to_ascii_lowercase())
    }
}

pub(crate) fn strip_www(host: &str) -> &str {
    host.strip_prefix("www.").unwrap_or(host)
}

pub(crate) fn normalize_path(raw_path: &str) -> String {
    let segments: Vec<&str> = raw_path.split('/').filter(|seg| !seg.is_empty()).collect();
    if segments.is_empty() {
        "/".to_string()
    } else {
        format!("/{}", segments.join("/"))
    }
}

fn assign_utm(utm: &mut UtmParams, key_lower: &str, raw_val: &str) -> bool {
    let trimmed = raw_val.trim();
    let val = (!trimmed.is_empty()).then(|| truncate_utf8(trimmed, MAX_UTM_BYTES).to_string());
    match key_lower {
        "utm_source" => utm.source = val,
        "utm_medium" => utm.medium = val,
        "utm_campaign" => utm.campaign = val,
        "utm_content" => utm.content = val,
        "utm_term" => utm.term = val,
        _ => return false,
    }
    true
}
