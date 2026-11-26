#![forbid(unsafe_code)]
//! `DuckDbStore` query execution and [`AnalyticsQueryStore`] implementation.

mod breakdown;
mod filters;
mod rollup_summary;
pub(crate) mod summary;
mod timeseries;
mod window;

use std::time::Instant;

use rotameter_core::error::CoreError;
use rotameter_core::id::SiteId;
use rotameter_core::query::{Metric, QuerySpec};
use rotameter_core::storage::{
    AnalyticsQueryStore, BoxFuture, QueryMeta, QueryResult, RealtimeSnapshot,
};
use rotameter_core::types::Site;

use crate::store::DuckDbStore;

impl DuckDbStore {
    /// Synchronously executes a validated [`QuerySpec`] against `DuckDB` OLAP tables.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::InvalidQuery`] or [`CoreError::Storage`] on failure.
    pub fn query_sync(
        &self,
        site: &Site,
        spec: &QuerySpec,
        now_ms: i64,
    ) -> Result<QueryResult, CoreError> {
        spec.validate()?;
        let started = Instant::now();
        let tz = window::parse_timezone(&site.timezone)?;

        self.with_conn(|conn| {
            let (from_ms, to_ms) =
                window::resolve_window(conn, site.site_id, &spec.range, tz, now_ms)?;
            let kpi = summary::query_summary(conn, site, from_ms, to_ms, &spec.filters)?;

            let previous_summary = if spec.compare_previous {
                let (prev_from, prev_to) = window::previous_window(from_ms, to_ms);
                Some(summary::query_summary(
                    conn,
                    site,
                    prev_from,
                    prev_to,
                    &spec.filters,
                )?)
            } else {
                None
            };

            let ts_points = match spec.granularity {
                Some(granularity) => timeseries::query_timeseries(
                    conn,
                    site,
                    from_ms,
                    to_ms,
                    granularity,
                    tz,
                    &spec.filters,
                )?,
                None => Vec::new(),
            };

            let primary_metric = spec.metrics.first().copied().unwrap_or(Metric::Visitors);
            let bd = breakdown::query_breakdown(
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
                summary: kpi,
                previous_summary,
                timeseries: ts_points,
                rows: bd.rows,
                meta: QueryMeta {
                    total_rows: bd.total_rows,
                    query_ms: started.elapsed().as_secs_f64() * 1_000.0,
                    truncated_by_cardinality_cap: bd.truncated_by_cardinality_cap,
                },
            })
        })
    }
}

impl AnalyticsQueryStore for DuckDbStore {
    fn query<'a>(
        &'a self,
        site: &'a Site,
        spec: &'a QuerySpec,
        now_ms: i64,
    ) -> BoxFuture<'a, Result<QueryResult, CoreError>> {
        Box::pin(std::future::ready(self.query_sync(site, spec, now_ms)))
    }

    fn realtime<'a>(
        &'a self,
        site: &'a Site,
        now_ms: i64,
    ) -> BoxFuture<'a, Result<RealtimeSnapshot, CoreError>> {
        Box::pin(std::future::ready(self.realtime_sync(site, now_ms)))
    }

    fn purge_before(
        &self,
        site_id: SiteId,
        before_ms: i64,
    ) -> BoxFuture<'_, Result<u64, CoreError>> {
        Box::pin(std::future::ready(
            self.purge_before_sync(site_id, before_ms),
        ))
    }
}
