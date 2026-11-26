#![forbid(unsafe_code)]
//! `RFC 4180` CSV projection for [`QueryResult`] payloads.

use crate::query::{Dimension, Metric, QuerySpec};
use crate::storage::models::{BreakdownRow, KpiSummary, QueryResult};

/// Formats a [`QueryResult`] as an `RFC 4180` CSV string according to `spec`.
///
/// Exports breakdown rows when `spec.group_by` is non-empty, otherwise time-series
/// buckets when `spec.granularity` is set, and otherwise the aggregate summary row.
pub fn format_query_result_csv(result: &QueryResult, spec: &QuerySpec) -> String {
    if !spec.group_by.is_empty() {
        return format_breakdown_csv(result, spec);
    }
    if spec.granularity.is_some() {
        return format_timeseries_csv(result);
    }
    format_summary_csv(&result.summary, &spec.metrics)
}

fn format_breakdown_csv(result: &QueryResult, spec: &QuerySpec) -> String {
    let mut headers: Vec<&str> = spec
        .group_by
        .iter()
        .map(|d| dimension_key_name(*d))
        .collect();
    headers.extend(spec.metrics.iter().map(|m| metric_header(*m)));

    let mut out = String::new();
    write_csv_line(&mut out, &headers);

    for row in &result.rows {
        let mut fields = Vec::with_capacity(headers.len());
        for dim in &spec.group_by {
            let key = dimension_key_name(*dim);
            fields.push(row.key.get(key).cloned().unwrap_or_default());
        }
        for metric in &spec.metrics {
            fields.push(format_breakdown_metric(row, *metric));
        }
        let refs: Vec<&str> = fields.iter().map(String::as_str).collect();
        write_csv_line(&mut out, &refs);
    }
    out
}

fn format_timeseries_csv(result: &QueryResult) -> String {
    let mut out = String::new();
    write_csv_line(&mut out, &["bucket_ms", "visitors", "pageviews", "visits"]);
    for point in &result.timeseries {
        let b = point.bucket_ms.to_string();
        let v = point.visitors.to_string();
        let p = point.pageviews.to_string();
        let s = point.visits.to_string();
        write_csv_line(&mut out, &[&b, &v, &p, &s]);
    }
    out
}

fn format_summary_csv(summary: &KpiSummary, metrics: &[Metric]) -> String {
    let headers: Vec<&str> = metrics.iter().map(|m| metric_header(*m)).collect();
    let values: Vec<String> = metrics
        .iter()
        .map(|m| format_summary_metric(summary, *m))
        .collect();
    let refs: Vec<&str> = values.iter().map(String::as_str).collect();
    let mut out = String::new();
    write_csv_line(&mut out, &headers);
    write_csv_line(&mut out, &refs);
    out
}

const fn dimension_key_name(dim: Dimension) -> &'static str {
    match dim {
        Dimension::Path => "path",
        Dimension::EntryPage => "entry_page",
        Dimension::ExitPage => "exit_page",
        Dimension::Source => "source",
        Dimension::Referrer => "referrer",
        Dimension::Channel => "channel",
        Dimension::Country => "country",
        Dimension::Region => "region",
        Dimension::Browser => "browser",
        Dimension::Os => "os",
        Dimension::Device => "device",
        Dimension::Viewport => "viewport",
        Dimension::EventName => "event_name",
        Dimension::UtmSource => "utm_source",
        Dimension::UtmMedium => "utm_medium",
        Dimension::UtmCampaign => "utm_campaign",
    }
}

const fn metric_header(metric: Metric) -> &'static str {
    match metric {
        Metric::Visitors => "visitors",
        Metric::Pageviews => "pageviews",
        Metric::Visits => "visits",
        Metric::ViewsPerVisit => "views_per_visit",
        Metric::BounceRate => "bounce_rate",
        Metric::AvgVisitDurationMs => "avg_visit_duration_ms",
    }
}

#[allow(clippy::cast_precision_loss)]
const fn u64_to_f64(value: u64) -> f64 {
    value as f64
}

fn format_breakdown_metric(row: &BreakdownRow, metric: Metric) -> String {
    match metric {
        Metric::Visitors => row.visitors.to_string(),
        Metric::Pageviews => row.pageviews.to_string(),
        Metric::Visits => row.visits.to_string(),
        Metric::ViewsPerVisit => {
            let vpv = if row.visits == 0 {
                0.0
            } else {
                u64_to_f64(row.pageviews) / u64_to_f64(row.visits)
            };
            format!("{vpv:.4}")
        }
        Metric::BounceRate => format!("{:.4}", row.bounce_rate),
        Metric::AvgVisitDurationMs => row.avg_visit_duration_ms.to_string(),
    }
}

fn format_summary_metric(summary: &KpiSummary, metric: Metric) -> String {
    match metric {
        Metric::Visitors => summary.visitors.to_string(),
        Metric::Pageviews => summary.pageviews.to_string(),
        Metric::Visits => summary.visits.to_string(),
        Metric::ViewsPerVisit => format!("{:.4}", summary.views_per_visit),
        Metric::BounceRate => format!("{:.4}", summary.bounce_rate),
        Metric::AvgVisitDurationMs => summary.avg_visit_duration_ms.to_string(),
    }
}

fn write_csv_line(out: &mut String, fields: &[&str]) {
    for (idx, field) in fields.iter().enumerate() {
        if idx > 0 {
            out.push(',');
        }
        push_csv_field(out, field);
    }
    out.push('\n');
}

fn push_csv_field(out: &mut String, val: &str) {
    if val.contains([',', '"', '\n', '\r']) {
        out.push('"');
        for ch in val.chars() {
            if ch == '"' {
                out.push('"');
            }
            out.push(ch);
        }
        out.push('"');
    } else {
        out.push_str(val);
    }
}
