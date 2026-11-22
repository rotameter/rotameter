#![forbid(unsafe_code)]
//! Aggregate KPI summary orchestration and numeric conversion helpers.

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::privacy::MS_PER_HOUR;
use rotameter_core::query::QueryFilter;
use rotameter_core::storage::KpiSummary;
use rotameter_core::types::Site;
use rusqlite::{Connection, OptionalExtension, types::Value};

use crate::config::map_sql_err;
use crate::query::daily_fast::try_daily_summary;
use crate::query::filter_sql::build_filter_plan;
use crate::query::rollup_summary::{
    query_filtered_event_summary, query_historical_rollup, query_session_slice_raw,
    query_session_slice_summary,
};

#[derive(Default)]
pub(crate) struct RollupAgg {
    pub(crate) pageviews: u64,
    pub(crate) visits: u64,
    pub(crate) bounced_visits: u64,
    pub(crate) total_duration_ms: u64,
}

#[allow(clippy::cast_precision_loss)]
pub(crate) fn u64_to_f64(value: u64) -> f64 {
    value as f64
}

pub(crate) fn non_neg_u64(value: i64) -> u64 {
    u64::try_from(value.max(0)).unwrap_or(0)
}

pub(crate) fn build_kpi_summary(
    visitors: u64,
    pageviews: u64,
    visits: u64,
    bounced_visits: u64,
    total_duration_ms: u64,
) -> KpiSummary {
    let views_per_visit = if visits == 0 {
        0.0
    } else {
        u64_to_f64(pageviews) / u64_to_f64(visits)
    };
    let bounce_rate = if visits == 0 {
        0.0
    } else {
        (u64_to_f64(bounced_visits) / u64_to_f64(visits)).clamp(0.0, 1.0)
    };
    let avg_visit_duration_ms = total_duration_ms.checked_div(visits).unwrap_or(0);
    KpiSummary {
        visitors,
        pageviews,
        visits,
        views_per_visit,
        bounce_rate,
        avg_visit_duration_ms,
    }
}

pub(crate) fn query_summary(
    conn: &Connection,
    site: &Site,
    from_ms: i64,
    to_ms: i64,
    filters: &[QueryFilter],
) -> Result<KpiSummary, CoreError> {
    if filters.is_empty() {
        if let Some(fast) = try_daily_summary(conn, site, from_ms, to_ms)? {
            return Ok(fast);
        }
        return query_unfiltered_summary(conn, site.site_id, from_ms, to_ms);
    }

    let mut params = vec![
        Value::Blob(site.site_id.as_bytes().to_vec()),
        Value::Integer(from_ms),
        Value::Integer(to_ms),
    ];
    let plan = build_filter_plan(filters, &[], &mut params)?;
    let joins = plan.join_sql();
    let where_tail = plan.where_sql();

    if plan.needs_events {
        query_filtered_event_summary(conn, &joins, &where_tail, &params)
    } else {
        query_session_slice_summary(conn, &joins, &where_tail, &params)
    }
}

pub(crate) fn raw_session_hour_floor(
    conn: &Connection,
    site_id: SiteId,
    to_ms: i64,
) -> Result<(i64, bool), CoreError> {
    let min_started: Option<i64> = conn
        .query_row(
            "SELECT MIN(started_at_ms) FROM sessions WHERE site_id = ?1",
            [site_id.as_bytes().as_slice()],
            |r| r.get(0),
        )
        .optional()
        .map_err(map_sql_err)?
        .flatten();
    Ok(match min_started {
        Some(ts) => ((ts.div_euclid(MS_PER_HOUR) * MS_PER_HOUR).min(to_ms), true),
        None => (to_ms, false),
    })
}

fn query_unfiltered_summary(
    conn: &Connection,
    site_id: SiteId,
    from_ms: i64,
    to_ms: i64,
) -> Result<KpiSummary, CoreError> {
    let (raw_floor, has_live) = raw_session_hour_floor(conn, site_id, to_ms)?;
    let mut visitors = 0_u64;
    let mut agg = RollupAgg::default();

    if from_ms < raw_floor {
        let hist = query_historical_rollup(
            conn,
            site_id,
            from_ms,
            raw_floor.min(to_ms),
            raw_floor,
            has_live,
        )?;
        agg.pageviews += hist.pageviews;
        agg.visits += hist.visits;
        agg.bounced_visits += hist.bounced_visits;
        agg.total_duration_ms += hist.total_duration_ms;
    }

    let live_start = from_ms.max(raw_floor);
    if live_start < to_ms {
        let params = [
            Value::Blob(site_id.as_bytes().to_vec()),
            Value::Integer(live_start),
            Value::Integer(to_ms),
        ];
        let (live_visitors, live_agg) = query_session_slice_raw(conn, "", "", &params)?;
        visitors = live_visitors;
        agg.pageviews += live_agg.pageviews;
        agg.visits += live_agg.visits;
        agg.bounced_visits += live_agg.bounced_visits;
        agg.total_duration_ms += live_agg.total_duration_ms;
    }

    Ok(build_kpi_summary(
        visitors,
        agg.pageviews,
        agg.visits,
        agg.bounced_visits,
        agg.total_duration_ms,
    ))
}
