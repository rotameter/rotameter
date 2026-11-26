#![forbid(unsafe_code)]
//! Caddy structured JSON access log parser and syslog framing helpers.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use rotameter_client::types::{BatchEventItem, SiteRef};
use serde::Deserialize;

use crate::import_filter::classify_proxy_hit;
use crate::import_parse::normalize_import_url;

#[derive(Deserialize)]
pub(crate) struct CaddyLogLine {
    pub(crate) ts: Option<serde_json::Value>,
    pub(crate) request: CaddyRequest,
    pub(crate) status: u16,
    #[serde(default)]
    pub(crate) resp_headers: BTreeMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
pub(crate) struct CaddyRequest {
    pub(crate) method: String,
    pub(crate) host: Option<String>,
    pub(crate) uri: String,
    pub(crate) client_ip: Option<String>,
    pub(crate) remote_ip: Option<String>,
    #[serde(default)]
    pub(crate) headers: BTreeMap<String, serde_json::Value>,
}

/// Strips an optional RFC3164/RFC5424 syslog `<PRI>...tag: ` prefix before log parsing.
pub(crate) fn strip_syslog_prefix(line: &str) -> &str {
    let trimmed = line.trim();
    if !trimmed.starts_with('<') {
        return trimmed;
    }
    let Some(gt) = trimmed.find('>') else {
        return trimmed;
    };
    if gt <= 1 || !trimmed[1..gt].bytes().all(|b| b.is_ascii_digit()) {
        return trimmed;
    }
    let after_pri = &trimmed[gt + 1..];
    if let Some(colon_pos) = after_pri.find(": ") {
        let candidate = after_pri[colon_pos + 2..].trim_start();
        if !candidate.is_empty() {
            return candidate;
        }
    }
    trimmed
}

pub(crate) fn parse_caddy_json_line(
    site_ref: &SiteRef,
    default_host: &str,
    line: &str,
    apply_proxy_filter: bool,
) -> Option<BatchEventItem> {
    let entry: CaddyLogLine = serde_json::from_str(line).ok()?;
    let content_type = header_first(&entry.resp_headers, "content-type");
    let event = if apply_proxy_filter {
        classify_proxy_hit(
            &entry.request.method,
            entry.status,
            &entry.request.uri,
            content_type.as_deref(),
        )?
    } else if entry.status == 404 {
        "not_found"
    } else {
        "pageview"
    };

    let host = entry
        .request
        .host
        .as_deref()
        .map(str::trim)
        .filter(|h| !h.is_empty())
        .unwrap_or(default_host);
    let url = normalize_import_url(host, &entry.request.uri)?;
    let ts = entry.ts.as_ref().and_then(parse_caddy_ts);
    let referrer = header_first(&entry.request.headers, "referer");
    let user_agent = header_first(&entry.request.headers, "user-agent");
    let ip = entry
        .request
        .client_ip
        .as_deref()
        .or(entry.request.remote_ip.as_deref())
        .and_then(strip_port);

    Some(BatchEventItem {
        site_id: site_ref.clone(),
        event: Some(event.to_string()),
        url,
        referrer,
        viewport_width: None,
        props: None,
        ts,
        visitor_hint: None,
        user_agent,
        country: None,
        ip,
    })
}

fn parse_caddy_ts(val: &serde_json::Value) -> Option<String> {
    match val {
        serde_json::Value::Number(num) => {
            let secs = num.as_f64()?;
            let millis = (secs * 1_000.0).round();
            if !millis.is_finite() {
                return None;
            }
            #[allow(clippy::cast_possible_truncation)]
            let ms = millis as i64;
            DateTime::from_timestamp_millis(ms).map(|dt| dt.with_timezone(&Utc).to_rfc3339())
        }
        serde_json::Value::String(s) => DateTime::parse_from_rfc3339(s.trim())
            .ok()
            .map(|dt| dt.with_timezone(&Utc).to_rfc3339()),
        _ => None,
    }
}

fn header_first(headers: &BTreeMap<String, serde_json::Value>, target: &str) -> Option<String> {
    let val = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(target))?
        .1;
    match val {
        serde_json::Value::String(s) => clean_non_empty(s),
        serde_json::Value::Array(arr) => arr
            .first()
            .and_then(|v| v.as_str())
            .and_then(clean_non_empty),
        _ => None,
    }
}

fn clean_non_empty(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    (!trimmed.is_empty() && trimmed != "-").then(|| trimmed.to_string())
}

fn strip_port(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "-" {
        return None;
    }
    if let Ok(addr) = trimmed.parse::<std::net::SocketAddr>() {
        return Some(addr.ip().to_string());
    }
    Some(trimmed.to_string())
}

#[cfg(test)]
mod tests {
    use rotameter_client::types::SiteRef;

    use super::{parse_caddy_json_line, strip_syslog_prefix};

    #[test]
    fn strips_syslog_prefix_and_parses_caddy_json_with_proxy_filter() {
        let raw = r#"<190>Nov 26 10:00:00 edge caddy: {"ts":1700000000.5,"request":{"method":"GET","host":"example.com","uri":"/docs?q=1","client_ip":"203.0.113.5:51432","headers":{"User-Agent":["Mozilla/5.0 Chrome/131.0.0.0"],"Referer":["https://news.ycombinator.com/"]}},"status":200,"resp_headers":{"Content-Type":["text/html; charset=utf-8"]}}"#;
        let stripped = strip_syslog_prefix(raw);
        assert!(stripped.starts_with('{'));

        let site = SiteRef::Slug("example.com".to_string());
        let item = parse_caddy_json_line(&site, "example.com", stripped, true).unwrap();
        assert_eq!(item.event.as_deref(), Some("pageview"));
        assert_eq!(item.url, "https://example.com/docs?q=1");
        assert_eq!(item.ip.as_deref(), Some("203.0.113.5"));
        assert_eq!(
            item.user_agent.as_deref(),
            Some("Mozilla/5.0 Chrome/131.0.0.0")
        );

        let asset = r#"{"ts":1700000000,"request":{"method":"GET","host":"example.com","uri":"/app.js"},"status":200,"resp_headers":{"Content-Type":["application/javascript"]}}"#;
        assert!(parse_caddy_json_line(&site, "example.com", asset, true).is_none());

        let nf = r#"{"ts":1700000000,"request":{"method":"GET","host":"example.com","uri":"/missing"},"status":404,"resp_headers":{"Content-Type":["text/html"]}}"#;
        let nf_item = parse_caddy_json_line(&site, "example.com", nf, true).unwrap();
        assert_eq!(nf_item.event.as_deref(), Some("not_found"));
    }
}
