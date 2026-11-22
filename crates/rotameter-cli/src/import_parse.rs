#![forbid(unsafe_code)]
//! `JSONL` and Nginx/Caddy Common/Combined Log Format (`CLF`) parsers for `rotameter import`.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use rotameter_client::types::{BatchEventItem, SiteRef};
use serde::Deserialize;
use url::Url;

use crate::cli_sub::ImportFormat;
use crate::exit::CliError;

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
}

pub(crate) fn parse_import_file(
    site_ref: &SiteRef,
    path: &Path,
    format: ImportFormat,
) -> Result<ParsedImport, CliError> {
    let raw = std::fs::read_to_string(path)
        .map_err(|err| CliError::runtime(format!("failed to read `{}`: {err}", path.display())))?;
    let host = default_host_for_site(site_ref);
    let mut items = Vec::new();
    let mut skipped = 0_usize;

    for line in raw.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let parsed = match resolve_line_format(line, format) {
            ImportFormat::Jsonl => parse_jsonl_line(site_ref, &host, line),
            ImportFormat::Clf => parse_clf_line(site_ref, &host, line),
            ImportFormat::Auto => None,
        };
        match parsed {
            Some(item) => items.push(item),
            None => skipped += 1,
        }
    }

    Ok(ParsedImport { items, skipped })
}

fn resolve_line_format(line: &str, format: ImportFormat) -> ImportFormat {
    match format {
        ImportFormat::Auto => {
            if line.starts_with('{') {
                ImportFormat::Jsonl
            } else {
                ImportFormat::Clf
            }
        }
        explicit => explicit,
    }
}

fn default_host_for_site(site_ref: &SiteRef) -> String {
    match site_ref {
        SiteRef::Slug(slug) if slug != "_self" => slug.clone(),
        _ => "import.rotameter.local".to_string(),
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
    })
}

fn parse_clf_line(site_ref: &SiteRef, host: &str, line: &str) -> Option<BatchEventItem> {
    let ip = line.split_whitespace().next()?;
    let lb = line.find('[')?;
    let rb = line[lb + 1..].find(']')?;
    let ts_raw = &line[lb + 1..lb + 1 + rb];
    let dt = DateTime::parse_from_str(ts_raw.trim(), "%d/%b/%Y:%H:%M:%S %z").ok()?;
    let ts = dt.with_timezone(&Utc).to_rfc3339();

    let rest = &line[lb + 1 + rb + 1..];
    let quotes = extract_quoted_fields(rest);
    let req_line = quotes.first()?;
    let target = req_line.split_whitespace().nth(1)?;
    let url = normalize_import_url(host, target)?;

    let referrer = quotes
        .get(1)
        .map(String::as_str)
        .filter(|s| !s.is_empty() && *s != "-")
        .map(ToString::to_string);
    let ua = quotes
        .get(2)
        .map(String::as_str)
        .filter(|s| !s.is_empty() && *s != "-")
        .unwrap_or("");

    Some(BatchEventItem {
        site_id: site_ref.clone(),
        event: Some("pageview".to_string()),
        url,
        referrer,
        viewport_width: None,
        props: None,
        ts: Some(ts),
        visitor_hint: Some(format!("{ip}|{ua}")),
    })
}

fn normalize_import_url(host: &str, raw_target: &str) -> Option<String> {
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

fn extract_quoted_fields(input: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_quotes = false;
    let mut escaped = false;
    let mut current = String::new();

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
        } else if in_quotes && ch == '\\' {
            escaped = true;
        } else if ch == '"' {
            if in_quotes {
                fields.push(std::mem::take(&mut current));
            }
            in_quotes = !in_quotes;
        } else if in_quotes {
            current.push(ch);
        }
    }
    fields
}
