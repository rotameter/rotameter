#![forbid(unsafe_code)]
//! `SQLite` implementation of [`AnalyticsQueryStore`] and CSV formatting helpers.

mod breakdown;
mod breakdown_sql;
pub mod csv;
mod daily_breakdown;
mod daily_fast;
mod dim_map;
mod filter_sql;
mod rollup_summary;
mod summary;
mod timeseries;
mod timeseries_rollup;
mod window;

use std::time::Instant;

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::query::{Metric, QuerySpec};
use rotameter_core::storage::{AnalyticsQueryStore, QueryMeta, QueryResult, RealtimeSnapshot};
use rotameter_core::types::Site;

pub use csv::format_query_result_csv;
pub(crate) use summary::non_neg_u64;

use self::breakdown::query_breakdown;
use self::summary::query_summary;
use self::timeseries::query_timeseries;
use self::window::{parse_timezone, previous_window, resolve_window};
use crate::realtime::query_realtime;
use crate::retention::purge_site_before;
use crate::store::SqliteStore;

impl AnalyticsQueryStore for SqliteStore {
    fn query(&self, site: &Site, spec: &QuerySpec, now_ms: i64) -> Result<QueryResult, CoreError> {
        spec.validate()?;
        let started = Instant::now();
        let tz = parse_timezone(&site.timezone)?;

        self.with_reader(|conn| {
            let (from_ms, to_ms) = resolve_window(conn, site.site_id, &spec.range, tz, now_ms)?;
            let summary = query_summary(conn, site, from_ms, to_ms, &spec.filters)?;

            let previous_summary = if spec.compare_previous {
                let (prev_from, prev_to) = previous_window(from_ms, to_ms);
                Some(query_summary(
                    conn,
                    site,
                    prev_from,
                    prev_to,
                    &spec.filters,
                )?)
            } else {
                None
            };

            let timeseries = match spec.granularity {
                Some(granularity) => {
                    query_timeseries(conn, site, from_ms, to_ms, granularity, tz, &spec.filters)?
                }
                None => Vec::new(),
            };

            let primary_metric = spec.metrics.first().copied().unwrap_or(Metric::Visitors);
            let breakdown = query_breakdown(
                conn,
                site,
                from_ms,
                to_ms,
                &spec.group_by,
                &spec.filters,
                primary_metric,
                spec.sort.as_ref(),
                spec.limit,
                spec.offset,
            )?;

            Ok(QueryResult {
                site_id: site.site_id,
                slug: site.slug.clone(),
                timezone: site.timezone.clone(),
                window_from_ms: from_ms,
                window_to_ms: to_ms,
                summary,
                previous_summary,
                timeseries,
                rows: breakdown.rows,
                meta: QueryMeta {
                    total_rows: breakdown.total_rows,
                    query_ms: started.elapsed().as_secs_f64() * 1_000.0,
                    truncated_by_cardinality_cap: breakdown.truncated_by_cardinality_cap,
                },
            })
        })
    }

    fn realtime(&self, site: &Site, now_ms: i64) -> Result<RealtimeSnapshot, CoreError> {
        self.with_reader(|conn| query_realtime(conn, site, now_ms))
    }

    fn purge_before(&self, site_id: SiteId, before_ms: i64) -> Result<u64, CoreError> {
        self.with_writer(|state| purge_site_before(&mut state.conn, site_id, before_ms))
    }
}
