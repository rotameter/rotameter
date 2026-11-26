#![forbid(unsafe_code)]
//! `SQLite` implementation of [`AnalyticsDataPlane`] and CSV formatting re-export.

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
use rotameter_core::storage::{
    AnalyticsDataPlane, AnalyticsQueryStore, BoxFuture, QueryMeta, QueryResult, RealtimeSnapshot,
    StorageCapabilities,
};
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

impl SqliteStore {
    /// Synchronously executes a validated [`QuerySpec`] against stored sessions, events, and rollups.
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

    /// Synchronously reads the trailing 5-minute active snapshot and 30-minute pulse for `site`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on query failure.
    pub fn realtime_sync(&self, site: &Site, now_ms: i64) -> Result<RealtimeSnapshot, CoreError> {
        self.with_reader(|conn| query_realtime(conn, site, now_ms))
    }

    /// Synchronously deletes raw and rollup analytics for `site_id` strictly older than `before_ms`.
    ///
    /// # Errors
    ///
    /// Returns [`CoreError::Storage`] on purge failure.
    pub fn purge_before_sync(&self, site_id: SiteId, before_ms: i64) -> Result<u64, CoreError> {
        self.with_writer(|state| purge_site_before(&mut state.conn, site_id, before_ms))
    }
}

impl AnalyticsQueryStore for SqliteStore {
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

impl AnalyticsDataPlane for SqliteStore {
    fn data_plane_capabilities(&self) -> StorageCapabilities {
        self.capabilities_sync()
    }

    fn flush_shutdown(&self) -> BoxFuture<'_, Result<(), CoreError>> {
        Box::pin(std::future::ready(self.checkpoint_truncate()))
    }
}
