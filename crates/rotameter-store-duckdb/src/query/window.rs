#![forbid(unsafe_code)]
//! Time-window resolution, comparison windows, and IANA timezone bucketing for `DuckDB`.

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use duckdb::{Connection, OptionalExt};
use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::{MS_PER_DAY, MS_PER_HOUR};
use rotameter_core::query::{Granularity, RangePreset, TimeRange};

use crate::store::map_duck_err;

pub(crate) fn parse_timezone(tz_name: &str) -> Result<Tz, CoreError> {
    tz_name
        .parse::<Tz>()
        .map_err(|err| CoreError::InvalidQuery(format!("invalid site timezone `{tz_name}`: {err}")))
}

pub(crate) fn resolve_window(
    conn: &Connection,
    site_id: SiteId,
    range: &TimeRange,
    tz: Tz,
    now_ms: i64,
) -> Result<(i64, i64), CoreError> {
    match range {
        TimeRange::Explicit { from_ms, to_ms } => Ok((*from_ms, *to_ms)),
        TimeRange::Preset { preset } => match preset {
            RangePreset::Hours24 => Ok((now_ms.saturating_sub(24 * MS_PER_HOUR), now_ms)),
            RangePreset::Days7 => Ok((now_ms.saturating_sub(7 * MS_PER_DAY), now_ms)),
            RangePreset::Days30 => Ok((now_ms.saturating_sub(30 * MS_PER_DAY), now_ms)),
            RangePreset::Months12 => Ok((minus_12_months_ms(now_ms, tz), now_ms)),
            RangePreset::All => resolve_all_window(conn, site_id, now_ms),
        },
    }
}

pub(crate) fn previous_window(from_ms: i64, to_ms: i64) -> (i64, i64) {
    let span = to_ms.saturating_sub(from_ms);
    (from_ms.saturating_sub(span), from_ms)
}

pub(crate) fn floor_bucket_ms(ts_ms: i64, granularity: Granularity, tz: Tz) -> i64 {
    let Some(utc_dt) = DateTime::from_timestamp_millis(ts_ms) else {
        return ts_ms.div_euclid(MS_PER_HOUR) * MS_PER_HOUR;
    };
    let local = utc_dt.with_timezone(&tz);
    match granularity {
        Granularity::Hour => resolve_local_ms(
            tz,
            local.year(),
            local.month(),
            local.day(),
            local.hour(),
            0,
            0,
        ),
        Granularity::Day => resolve_local_ms(tz, local.year(), local.month(), local.day(), 0, 0, 0),
        Granularity::Month => resolve_local_ms(tz, local.year(), local.month(), 1, 0, 0, 0),
    }
}

pub(crate) fn utc_day_start_ms(day_utc: i32) -> Option<i64> {
    let year = day_utc.div_euclid(10_000);
    let rem = day_utc.rem_euclid(10_000);
    let month = u32::try_from(rem.div_euclid(100)).ok()?;
    let day = u32::try_from(rem.rem_euclid(100)).ok()?;
    NaiveDate::from_ymd_opt(year, month, day)
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp_millis())
}

fn resolve_all_window(
    conn: &Connection,
    site_id: SiteId,
    now_ms: i64,
) -> Result<(i64, i64), CoreError> {
    let site_blob = site_id.as_bytes().as_slice();
    let min_session: Option<i64> = conn
        .query_row(
            "SELECT MIN(started_at_ms) FROM sessions WHERE site_id = ?1",
            [site_blob],
            |r| r.get(0),
        )
        .optional()
        .map_err(map_duck_err)?
        .flatten();
    let min_hourly: Option<i64> = conn
        .query_row(
            "SELECT MIN(bucket_ts_ms) FROM rollups_hourly WHERE site_id = ?1",
            [site_blob],
            |r| r.get(0),
        )
        .optional()
        .map_err(map_duck_err)?
        .flatten();
    let min_daily_key: Option<i32> = conn
        .query_row(
            "SELECT MIN(day_utc) FROM rollups_daily WHERE site_id = ?1",
            [site_blob],
            |r| r.get(0),
        )
        .optional()
        .map_err(map_duck_err)?
        .flatten();
    let min_daily = min_daily_key.and_then(utc_day_start_ms);

    let earliest = [min_session, min_hourly, min_daily]
        .into_iter()
        .flatten()
        .min();
    match earliest {
        Some(min_ms) => Ok((min_ms, now_ms.max(min_ms.saturating_add(1)))),
        None => Ok((now_ms.saturating_sub(MS_PER_DAY), now_ms)),
    }
}

fn minus_12_months_ms(now_ms: i64, tz: Tz) -> i64 {
    let Some(utc_dt) = DateTime::from_timestamp_millis(now_ms) else {
        return now_ms.saturating_sub(365 * MS_PER_DAY);
    };
    let local = utc_dt.with_timezone(&tz);
    let target_year = local.year().saturating_sub(1);
    let mut day = local.day();
    while NaiveDate::from_ymd_opt(target_year, local.month(), day).is_none() && day > 1 {
        day -= 1;
    }
    resolve_local_ms(
        tz,
        target_year,
        local.month(),
        day,
        local.hour(),
        local.minute(),
        local.second(),
    )
}

fn resolve_local_ms(
    tz: Tz,
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
) -> i64 {
    if let Some(dt) = tz
        .with_ymd_and_hms(year, month, day, hour, minute, second)
        .earliest()
    {
        return dt.timestamp_millis();
    }
    if let Some(dt) = tz
        .with_ymd_and_hms(year, month, day, (hour + 1) % 24, minute, second)
        .earliest()
    {
        return dt.timestamp_millis();
    }
    Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
        .earliest()
        .map_or(0, |dt| dt.timestamp_millis())
}
