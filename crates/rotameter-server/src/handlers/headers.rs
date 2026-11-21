#![forbid(unsafe_code)]
//! HTTP header extraction helpers for client IP, Cloudflare `GeoIP`, and event props.

use std::collections::BTreeMap;

use axum::http::HeaderMap;
use rotameter_core::types::EventProps;

use crate::error::ApiError;

pub(crate) fn header_str(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

pub(crate) fn extract_client_ip(headers: &HeaderMap) -> String {
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

pub(crate) fn extract_country(headers: &HeaderMap) -> String {
    header_str(headers, "cf-ipcountry")
        .map(|s| s.to_ascii_uppercase())
        .filter(|s| !matches!(s.as_str(), "XX" | "T1" | ""))
        .unwrap_or_else(|| "(unknown)".to_string())
}

pub(crate) fn extract_region(headers: &HeaderMap) -> String {
    header_str(headers, "cf-region-code")
        .or_else(|| header_str(headers, "cf-region"))
        .unwrap_or_else(|| "(unknown)".to_string())
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
