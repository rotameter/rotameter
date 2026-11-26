#![forbid(unsafe_code)]
//! Streaming `JSONL`, `caddy-json`, and `CLF` parsers for `rotameter import`.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use chrono::{DateTime, Utc};
use rotameter_client::types::{BatchEventItem, SiteRef};
use serde::Deserialize;
use url::Url;

use crate::cli_sub::ImportFormat;
use crate::exit::CliError;
use crate::import_caddy::{parse_caddy_json_line, strip_syslog_prefix};
use crate::import_filter::classify_proxy_hit;

pub(crate) struct ParsedImport {
    pub(crate) items: Vec<BatchEventItem>,
    pub(crate) skipped: usize,
}

#[derive(Deserialize)]
struct JsonlHit {
    url: String,
    event: Option<String>,
    referrer: Option<String>,
    viewport_width: Option<u16>,
    props: Option<BTreeMap<String, serde_json::Value>>,
    ts: Option<String>,
    visitor_hint: Option<String>,
    user_agent: Option<String>,
    country: Option<String>,
    ip: Option<String>,
}

pub(crate) fn parse_import_file(
    site_ref: &SiteRef,
    path: &Path,
    format: ImportFormat,
    apply_proxy_filter: bool,
) -> Result<ParsedImport, CliError> {
    let file = File::open(path)
        .map_err(|err| CliError::runtime(format!("failed to open `{}`: {err}", path.display())))?;
    parse_import_reader(site_ref, BufReader::new(file), format, apply_proxy_filter)
}

pub(crate) fn parse_import_reader<R: BufRead>(
    site_ref: &SiteRef,
    reader: R,
    format: ImportFormat,
    apply_proxy_filter: bool,
) -> Result<ParsedImport, CliError> {
    let host = default_host_for_site(site_ref);
    let mut items = Vec::new();
    let mut skipped = 0_usize;

    for line_res in reader.lines() {
        let line = line_res
            .map_err(|err| CliError::runtime(format!("failed reading import stream: {err}")))?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        match parse_import_line(site_ref, &host, trimmed, format, apply_proxy_filter) {
            Some(item) => items.push(item),
            None => skipped += 1,
        }
    }
    Ok(ParsedImport { items, skipped })
}

pub(crate) fn parse_import_line(
    site_ref: &SiteRef,
    host: &str,
    raw_line: &str,
    format: ImportFormat,
    apply_proxy_filter: bool,
) -> Option<BatchEventItem> {
    let line = strip_syslog_prefix(raw_line);
    if line.is_empty() {
        return None;
    }
    match resolve_line_format(line, format) {
        ImportFormat::Jsonl => parse_jsonl_line(site_ref, host, line),
        ImportFormat::CaddyJson => parse_caddy_json_line(site_ref, host, line, apply_proxy_filter),
        ImportFormat::Clf => parse_clf_line(site_ref, host, line, apply_proxy_filter),
        ImportFormat::Auto => None,
    }
}

pub(crate) fn default_host_for_site(site_ref: &SiteRef) -> String {
    match site_ref {
        SiteRef::Slug(slug) if slug != "_self" => slug.clone(),
        _ => "import.rotameter.local".to_string(),
    }
}

fn resolve_line_format(line: &str, format: ImportFormat) -> ImportFormat {
    match format {
        ImportFormat::Auto => {
            if line.starts_with('{') {
                if line.contains("\"request\"") && line.contains("\"status\"") {
                    ImportFormat::CaddyJson
                } else {
                    ImportFormat::Jsonl
                }
            } else {
                ImportFormat::Clf
            }
        }
        explicit => explicit,
    }
}

fn parse_jsonl_line(site_ref: &SiteRef, host: &str, line: &str) -> Option<BatchEventItem> {
    let hit: JsonlHit = serde_json::from_str(line).ok()?;
    if let Some(ts) = hit.ts.as_deref() {
        DateTime::parse_from_rfc3339(ts.trim()).ok()?;
    }
    let url = normalize_import_url(host, &hit.url)?;
    Some(BatchEventItem {
        site_id: site_ref.clone(),
        event: hit.event,
        url,
        referrer: hit.referrer,
        viewport_width: hit.viewport_width,
        props: hit.props,
        ts: hit.ts,
        visitor_hint: hit.visitor_hint,
        user_agent: hit.user_agent,
        country: hit.country,
        ip: hit.ip,
    })
}

fn parse_clf_line(
    site_ref: &SiteRef,
    host: &str,
    line: &str,
    apply_proxy_filter: bool,
) -> Option<BatchEventItem> {
    let ip_raw = line.split_whitespace().next()?;
    let lb = line.find('[')?;
    let rb = line[lb + 1..].find(']')?;
    let ts_raw = &line[lb + 1..lb + 1 + rb];
    let dt = DateTime::parse_from_str(ts_raw.trim(), "%d/%b/%Y:%H:%M:%S %z").ok()?;
    let ts = dt.with_timezone(&Utc).to_rfc3339();

    let rest = &line[lb + 1 + rb + 1..];
    let (quotes, status) = extract_quoted_and_status(rest);
    let req_line = quotes.first()?;
    let mut req_parts = req_line.split_whitespace();
    let method = req_parts.next()?;
    let target = req_parts.next()?;

    let event = if apply_proxy_filter {
        let code = status?;
        classify_proxy_hit(method, code, target, None)?
    } else if status == Some(404) {
        "not_found"
    } else {
        "pageview"
    };
    let url = normalize_import_url(host, target)?;

    let referrer = quotes
        .get(1)
        .map(String::as_str)
        .filter(|s| !s.is_empty() && *s != "-")
        .map(ToString::to_string);
    let user_agent = quotes
        .get(2)
        .map(String::as_str)
        .filter(|s| !s.is_empty() && *s != "-")
        .map(ToString::to_string);
    let ip = (!ip_raw.is_empty() && ip_raw != "-").then(|| ip_raw.to_string());

    Some(BatchEventItem {
        site_id: site_ref.clone(),
        event: Some(event.to_string()),
        url,
        referrer,
        viewport_width: None,
        props: None,
        ts: Some(ts),
        visitor_hint: None,
        user_agent,
        country: None,
        ip,
    })
}

pub(crate) fn normalize_import_url(host: &str, raw_target: &str) -> Option<String> {
    let trimmed = raw_target.trim();
    if trimmed.is_empty() {
        return None;
    }
    let candidate = if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else if trimmed.starts_with('/') {
        format!("https://{host}{trimmed}")
    } else {
        format!("https://{host}/{trimmed}")
    };
    let parsed = Url::parse(&candidate).ok()?;
    matches!(parsed.scheme(), "http" | "https").then(|| parsed.to_string())
}

fn extract_quoted_and_status(input: &str) -> (Vec<String>, Option<u16>) {
    let mut fields = Vec::new();
    let mut in_quotes = false;
    let mut escaped = false;
    let mut current = String::new();
    let mut after_first_quote_end: Option<usize> = None;

    for (idx, ch) in input.char_indices() {
        if escaped {
            current.push(ch);
            escaped = false;
        } else if in_quotes && ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            if in_quotes {
                fields.push(std::mem::take(&mut current));
                if fields.len() == 1 {
                    after_first_quote_end = Some(idx + 1);
                }
            }
            in_quotes = !in_quotes;
        } else if in_quotes {
            current.push(ch);
        }
    }
    let status = after_first_quote_end
        .and_then(|pos| input[pos..].split_whitespace().next())
        .and_then(|tok| tok.parse::<u16>().ok());
    (fields, status)
}
