#![forbid(unsafe_code)]
//! HTTP header extraction helpers for client IP, Cloudflare `GeoIP`, and event props.

use std::collections::BTreeMap;

use axum::http::HeaderMap;
use rotameter_core::bot::PrefetchHeaders;
use rotameter_core::types::EventProps;

use crate::error::ApiError;

pub(crate) fn extract_prefetch(headers: &HeaderMap) -> PrefetchHeaders<'_> {
    PrefetchHeaders {
        sec_purpose: headers.get("sec-purpose").and_then(|v| v.to_str().ok()),
        purpose: headers.get("purpose").and_then(|v| v.to_str().ok()),
        x_moz: headers.get("x-moz").and_then(|v| v.to_str().ok()),
    }
}

pub(crate) fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub(crate) fn extract_client_ip(headers: &HeaderMap, trusted_proxy: bool) -> String {
    if trusted_proxy && let Some(ip) = header_str(headers, "x-rotameter-client-ip") {
        return ip;
    }
    if let Some(ip) = header_str(headers, "cf-connecting-ip") {
        return ip;
    }
    if let Some(xff) = header_str(headers, "x-forwarded-for")
        && let Some(first) = xff
            .split(',')
            .next()
            .map(str::trim)
            .filter(|s| !s.is_empty())
    {
        return first.to_string();
    }
    header_str(headers, "x-real-ip").unwrap_or_else(|| "0.0.0.0".to_string())
}

pub(crate) fn extract_country(headers: &HeaderMap, trusted_proxy: bool) -> String {
    let trusted = if trusted_proxy {
        normalize_country(header_str(headers, "x-rotameter-country"))
    } else {
        None
    };
    trusted
        .or_else(|| normalize_country(header_str(headers, "cf-ipcountry")))
        .unwrap_or_else(|| "(unknown)".to_string())
}

pub(crate) fn extract_region(headers: &HeaderMap, trusted_proxy: bool) -> String {
    let trusted = if trusted_proxy {
        header_str(headers, "x-rotameter-region")
    } else {
        None
    };
    trusted
        .or_else(|| header_str(headers, "cf-region-code"))
        .or_else(|| header_str(headers, "cf-region"))
        .unwrap_or_else(|| "(unknown)".to_string())
}

pub(crate) fn normalize_country(raw: Option<String>) -> Option<String> {
    raw.map(|s| s.to_ascii_uppercase())
        .filter(|s| !matches!(s.as_str(), "XX" | "T1" | ""))
}

pub(crate) fn parse_props(
    raw: Option<&BTreeMap<String, serde_json::Value>>,
) -> Result<EventProps, ApiError> {
    let Some(map) = raw else {
        return Ok(EventProps::default());
    };
    let mut pairs = Vec::with_capacity(map.len());
    for (k, v) in map {
        let s = match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Null => continue,
            _ => {
                return Err(ApiError::bad_request(
                    "invalid_properties",
                    "custom property values must be JSON scalars",
                ));
            }
        };
        pairs.push((k.as_str(), s));
    }
    EventProps::from_pairs(pairs).map_err(ApiError::from)
}
