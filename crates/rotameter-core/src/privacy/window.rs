#![forbid(unsafe_code)]
//! Pure-Rust UTC salt window calculation and midnight grace carryover helpers.

use crate::types::SaltRotation;

/// Milliseconds in one UTC hour.
pub const MS_PER_HOUR: i64 = 3_600_000;
/// Milliseconds in one UTC day.
pub const MS_PER_DAY: i64 = 86_400_000;
/// Grace window after salt rotation (`30 minutes`) to preserve active sessions.
pub const GRACE_WINDOW_MS: i64 = 1_800_000;

/// Active or grace-window salt epoch for a given rotation cadence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SaltWindow {
    /// Configured rotation cadence.
    pub rotation: SaltRotation,
    /// Monotonic period index (`day`, `ISO week`, or `year * 12 + month - 1`).
    pub window_id: i64,
    /// Inclusive UTC start timestamp of this window in milliseconds.
    pub start_ms: i64,
}

/// Resolved current salt window and optional previous window during the `30m` grace period.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowResolution {
    /// Salt window active at the inspected timestamp.
    pub current: SaltWindow,
    /// Preceding salt window when `0 <= ts_ms - current.start_ms < GRACE_WINDOW_MS`.
    pub previous_in_grace: Option<SaltWindow>,
}

/// Resolves the current [`SaltWindow`] and optional `30m` grace carryover window for `ts_ms`.
pub fn resolve_salt_window(rotation: SaltRotation, ts_ms: i64) -> WindowResolution {
    let day = ts_ms.div_euclid(MS_PER_DAY);
    let (current, previous) = match rotation {
        SaltRotation::Daily => (
            SaltWindow {
                rotation,
                window_id: day,
                start_ms: day * MS_PER_DAY,
            },
            SaltWindow {
                rotation,
                window_id: day - 1,
                start_ms: (day - 1) * MS_PER_DAY,
            },
        ),
        SaltRotation::Weekly => {
            let week_id = (day + 3).div_euclid(7);
            let monday_day = week_id * 7 - 3;
            (
                SaltWindow {
                    rotation,
                    window_id: week_id,
                    start_ms: monday_day * MS_PER_DAY,
                },
                SaltWindow {
                    rotation,
                    window_id: week_id - 1,
                    start_ms: (monday_day - 7) * MS_PER_DAY,
                },
            )
        }
        SaltRotation::Monthly => {
            let (year, month, _) = civil_from_days(day);
            let window_id = i64::from(year) * 12 + i64::from(month - 1);
            let start_ms = days_from_civil(year, month, 1) * MS_PER_DAY;
            let (prev_year, prev_month) = if month == 1 {
                (year - 1, 12)
            } else {
                (year, month - 1)
            };
            (
                SaltWindow {
                    rotation,
                    window_id,
                    start_ms,
                },
                SaltWindow {
                    rotation,
                    window_id: window_id - 1,
                    start_ms: days_from_civil(prev_year, prev_month, 1) * MS_PER_DAY,
                },
            )
        }
    };

    let elapsed = ts_ms.saturating_sub(current.start_ms);
    let previous_in_grace = (0..GRACE_WINDOW_MS).contains(&elapsed).then_some(previous);

    WindowResolution {
        current,
        previous_in_grace,
    }
}

/// Formats `ts_ms` as an integer UTC `YYYYMMDD` date key for `rollups_daily`.
pub fn utc_yyyymmdd(ts_ms: i64) -> i32 {
    let (year, month, day) = civil_from_days(ts_ms.div_euclid(MS_PER_DAY));
    year * 10_000 + i32::from(month) * 100 + i32::from(day)
}

/// Floors `ts_ms` to the start of its UTC hour in milliseconds for `rollups_hourly`.
pub const fn utc_hour_floor_ms(ts_ms: i64) -> i64 {
    ts_ms.div_euclid(MS_PER_HOUR) * MS_PER_HOUR
}

fn civil_from_days(days: i64) -> (i32, u8, u8) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = mp + if mp < 10 { 3 } else { -9 };
    let year = y + i64::from(m <= 2);
    (
        i32::try_from(year).unwrap_or(i32::MAX),
        u8::try_from(m).unwrap_or(1),
        u8::try_from(d).unwrap_or(1),
    )
}

fn days_from_civil(year: i32, month: u8, day: u8) -> i64 {
    let y = i64::from(year) - i64::from(month <= 2);
    let m = i64::from(month);
    let d = i64::from(day);
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let adj_m = m + if m > 2 { -3 } else { 9 };
    let doy = (153 * adj_m + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}
