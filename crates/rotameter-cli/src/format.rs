#![forbid(unsafe_code)]
//! Table, `JSON`, and `RFC 4180` CSV output rendering plus timestamp formatting.

use std::fmt::Write;
#[cfg(feature = "sqlite")]
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(feature = "sqlite")]
use chrono::DateTime;
use clap::ValueEnum;
use serde::Serialize;

use crate::exit::CliError;

/// Output serialization format shared across CLI subcommands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable column-aligned table (default).
    #[default]
    Table,
    /// Pretty-printed JSON payload.
    Json,
    /// `RFC 4180` comma-separated values.
    Csv,
}

pub(crate) fn render_table(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if let Some(w) = widths.get_mut(i) {
                *w = (*w).max(cell.chars().count());
            }
        }
    }

    let mut out = String::new();
    append_table_row(&mut out, headers.iter().copied(), &widths);
    for (i, w) in widths.iter().enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        for _ in 0..*w {
            out.push('-');
        }
    }
    out.push('\n');
    for row in rows {
        append_table_row(&mut out, row.iter().map(String::as_str), &widths);
    }
    out
}

fn append_table_row<'a>(out: &mut String, cells: impl Iterator<Item = &'a str>, widths: &[usize]) {
    let collected: Vec<&str> = cells.collect();
    for (i, cell) in collected.iter().enumerate() {
        if i > 0 {
            out.push_str("  ");
        }
        if i + 1 == collected.len() {
            out.push_str(cell);
        } else {
            let width = widths.get(i).copied().unwrap_or(0);
            let _ = write!(out, "{cell:<width$}");
        }
    }
    out.push('\n');
}

pub(crate) fn render_csv(headers: &[&str], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    append_csv_row(&mut out, headers.iter().copied());
    for row in rows {
        append_csv_row(&mut out, row.iter().map(String::as_str));
    }
    out
}

fn append_csv_row<'a>(out: &mut String, cells: impl Iterator<Item = &'a str>) {
    for (i, cell) in cells.enumerate() {
        if i > 0 {
            out.push(',');
        }
        if cell.contains([',', '"', '\n', '\r']) {
            out.push('"');
            for ch in cell.chars() {
                if ch == '"' {
                    out.push('"');
                }
                out.push(ch);
            }
            out.push('"');
        } else {
            out.push_str(cell);
        }
    }
    out.push('\n');
}

pub(crate) fn render_json<T: Serialize>(value: &T) -> Result<String, CliError> {
    let mut text = serde_json::to_string_pretty(value)
        .map_err(|err| CliError::runtime(format!("JSON serialization error: {err}")))?;
    text.push('\n');
    Ok(text)
}

#[cfg(feature = "sqlite")]
pub(crate) fn format_rfc3339(ts_ms: i64) -> String {
    DateTime::from_timestamp_millis(ts_ms).map_or_else(
        || "1970-01-01T00:00:00Z".to_string(),
        |dt| dt.format("%Y-%m-%dT%H:%M:%SZ").to_string(),
    )
}

#[cfg(feature = "sqlite")]
pub(crate) fn now_ms() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
}
