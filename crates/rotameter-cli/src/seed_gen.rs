#![forbid(unsafe_code)]
//! Multi-day diurnal and trailing realtime schedule orchestrator for `rotameter seed`.

use rotameter_core::privacy::{MS_PER_DAY, MS_PER_HOUR, utc_hour_floor_ms};
use rotameter_core::storage::{EventIngestStore, WriteBatch};
use rotameter_core::types::Site;
use rotameter_store_sqlite::SqliteStore;

use crate::cmd_seed::SeedArgs;
use crate::exit::CliError;
use crate::seed_catalog::{DIURNAL_WEIGHTS, day_multiplier_permille};
use crate::seed_rng::SeedRng;
use crate::seed_session::{emit_capped_session, emit_session};

const MS_PER_MINUTE: i64 = 60_000;
const REALTIME_BUFFER_MS: i64 = 31 * MS_PER_MINUTE;

pub(crate) struct SeedStats {
    pub(crate) sessions: u64,
    pub(crate) events: u64,
    pub(crate) realtime_visitors: u64,
}

pub(crate) fn generate_and_write(
    store: &SqliteStore,
    site: &Site,
    now_ms: i64,
    args: &SeedArgs,
) -> Result<SeedStats, CliError> {
    let mut rng = SeedRng::new(args.seed);
    let mut batch = WriteBatch::default();
    let mut stats = SeedStats {
        sessions: 0,
        events: 0,
        realtime_visitors: u64::from(args.realtime_visitors),
    };

    let anchor_end = utc_hour_floor_ms(now_ms.saturating_sub(REALTIME_BUFFER_MS));
    let start_hour = anchor_end - i64::from(args.days) * 24 * MS_PER_HOUR;

    for day_idx in 0..args.days {
        let day_mult = day_multiplier_permille(day_idx, args.days);
        let day_sessions =
            u32::try_from((u64::from(args.sessions_per_day) * day_mult / 1_000).clamp(1, 200_000))
                .unwrap_or(1);
        let day_start = start_hour + i64::from(day_idx) * MS_PER_DAY;
        let hour_counts = distribute_day_hours(day_sessions, day_start, &mut rng);
        let mut visitors: Vec<[u8; 16]> = Vec::new();

        for (h_idx, &count) in hour_counts.iter().enumerate() {
            let h_start = day_start + i64::try_from(h_idx).unwrap_or(0) * MS_PER_HOUR;
            for _ in 0..count {
                let visitor_id = pick_day_visitor(&mut visitors, &mut rng);
                let ts_ms = rng.range_i64(h_start, h_start + MS_PER_HOUR - 240_000);
                emit_session(site, visitor_id, ts_ms, &mut rng, &mut batch, &mut stats);
                if batch.events.len() >= 2_000 {
                    store.write_batch(&batch)?;
                    batch = WriteBatch::default();
                }
            }
        }
    }

    emit_realtime_window(
        store,
        site,
        now_ms,
        args.realtime_visitors,
        &mut rng,
        &mut batch,
        &mut stats,
    )?;
    if !batch.events.is_empty() {
        store.write_batch(&batch)?;
    }
    Ok(stats)
}

fn distribute_day_hours(day_total: u32, day_start_ms: i64, rng: &mut SeedRng) -> [u32; 24] {
    let mut weights = [0_u64; 24];
    for (h, w) in weights.iter_mut().enumerate() {
        let h_ms = day_start_ms + i64::try_from(h).unwrap_or(0) * MS_PER_HOUR;
        let utc_h = usize::try_from((h_ms.div_euclid(MS_PER_HOUR)).rem_euclid(24)).unwrap_or(0);
        let jitter = u64::try_from(rng.range_i64(82, 118)).unwrap_or(100);
        *w = DIURNAL_WEIGHTS[utc_h] * jitter;
    }
    let total_w: u64 = weights.iter().sum::<u64>().max(1);
    let mut out = [0_u32; 24];
    let mut assigned = 0_u32;
    for (h, slot) in out.iter_mut().enumerate() {
        let share = u32::try_from(u64::from(day_total) * weights[h] / total_w).unwrap_or(0);
        *slot = share;
        assigned = assigned.saturating_add(share);
    }
    let mut cursor = 14_usize;
    while assigned < day_total {
        out[cursor % 24] += 1;
        assigned += 1;
        cursor = cursor.wrapping_add(3);
    }
    out
}

fn pick_day_visitor(pool: &mut Vec<[u8; 16]>, rng: &mut SeedRng) -> [u8; 16] {
    if !pool.is_empty() && rng.range_usize(100) < 22 {
        return pool[rng.range_usize(pool.len())];
    }
    let id = rng.next_bytes16();
    pool.push(id);
    id
}

fn emit_realtime_window(
    store: &SqliteStore,
    site: &Site,
    now_ms: i64,
    realtime_visitors: u32,
    rng: &mut SeedRng,
    batch: &mut WriteBatch,
    stats: &mut SeedStats,
) -> Result<(), CliError> {
    let pulse_start = now_ms.saturating_sub(29 * MS_PER_MINUTE);
    let pulse_end = now_ms.saturating_sub(6 * MS_PER_MINUTE);
    for _ in 0..24_u32 {
        let ts_ms = rng.range_i64(pulse_start, pulse_end);
        let vid = rng.next_bytes16();
        emit_session(site, vid, ts_ms, rng, batch, stats);
    }

    let live_start = now_ms.saturating_sub(240_000);
    let live_end = now_ms.saturating_sub(5_000);
    for _ in 0..realtime_visitors {
        let started = rng.range_i64(live_start, live_end.saturating_sub(3_000));
        let vid = rng.next_bytes16();
        emit_capped_session(
            site,
            vid,
            started,
            now_ms.saturating_sub(1_000),
            rng,
            batch,
            stats,
        );
        if batch.events.len() >= 2_000 {
            store.write_batch(batch)?;
            *batch = WriteBatch::default();
        }
    }
    Ok(())
}
