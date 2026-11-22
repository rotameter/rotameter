#![forbid(unsafe_code)]
//! Number, duration, delta, timestamp, and dimension-key formatting helpers.

use rotameter_client::types::{Dimension, RangePreset};

/// Formats an unsigned integer with US-style comma grouping (`1,842`).
pub fn format_count(n: u64) -> String {
    let raw = n.to_string();
    let mut out = String::with_capacity(raw.len() + raw.len() / 3);
    for (idx, ch) in raw.chars().rev().enumerate() {
        if idx > 0 && idx % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out.chars().rev().collect()
}

/// Formats milliseconds as `MMmSSs`.
pub fn format_duration(ms: u64) -> String {
    let secs = ms / 1_000;
    format!("{:02}m{:02}s", secs / 60, secs % 60)
}

/// Formats percentage delta (`+22%`, `-3%`, `--`).
pub fn format_delta(cur: f64, prev: f64) -> String {
    if prev.abs() <= f64::EPSILON {
        return "--".to_string();
    }
    let pct = ((cur - prev) / prev) * 100.0;
    format!("{pct:+.0}%")
}

/// Formats an ISO-8601 timestamp into compact date/hour readout labels.
pub fn short_bucket_label(iso: &str, range: RangePreset) -> String {
    let date = iso.get(0..10).unwrap_or(iso).replace('-', ".");
    if range == RangePreset::Hours24
        && let Some(hm) = iso.get(11..16)
    {
        return format!("{} {hm}", date.get(5..10).unwrap_or(&date));
    }
    date
}

/// Returns the canonical wire key string for a [`Dimension`].
pub const fn dimension_key(dim: Dimension) -> &'static str {
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
