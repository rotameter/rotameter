import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  ChartHost,
  useApiClient,
  useDashboardQuery,
  useSetTelemetryMeta,
  type PluginManifest,
} from "@rotameter/plugin-sdk";
import { InstrumentFrame } from "@rotameter/ui";
import { KpiStrip } from "./KpiStrip";
import { TimeSeriesChart } from "./TimeSeriesChart";

function OverviewPluginView() {
  const api = useApiClient();
  const q = useDashboardQuery();
  const setMeta = useSetTelemetryMeta();

  const { data, isPending, error } = useQuery({
    queryKey: ["overview", q.siteId, q.range, q.interval, q.compare, q.filters],
    queryFn: ({ signal }) =>
      api.query(
        {
          site_id: q.siteId,
          range: { preset: q.range },
          granularity: q.interval,
          metrics: [
            "visitors",
            "pageviews",
            "visits",
            "views_per_visit",
            "bounce_rate",
            "avg_visit_duration_ms",
          ],
          compare_previous: q.compare,
          filters: q.filters,
          limit: 50,
        },
        signal,
      ),
    staleTime: 30_000,
    enabled: Boolean(q.siteId),
  });

  useEffect(() => {
    if (data?.meta) setMeta(data.meta);
  }, [data?.meta, setMeta]);

  return (
    <div className="flex flex-col gap-[var(--density-grid-gap)]">
      <InstrumentFrame
        title="Overview Telemetry"
        subtitle={`${q.siteId} · ${q.range.toUpperCase()} · ${q.interval}`}
        reticles
      >
        {isPending ? (
          <p className="text-[var(--fg-muted)]">Loading site telemetry…</p>
        ) : error ? (
          <p className="text-[var(--signal-crit)]">{error.message}</p>
        ) : (
          <div className="flex flex-col gap-[var(--density-grid-gap)]">
            <KpiStrip summary={data.summary} previous={data.previous_summary} compare={q.compare} />
            <ChartHost
              vizType="timeseries"
              title="Traffic Trajectory (uPlot)"
              fallback={{
                caption: `Timeseries buckets for ${q.siteId}`,
                columns: [
                  { key: "bucket", label: "Bucket" },
                  { key: "visitors", label: "Visitors", align: "right" },
                  { key: "pageviews", label: "Pageviews", align: "right" },
                  { key: "visits", label: "Visits", align: "right" },
                ],
                rows: data.timeseries,
              }}
            >
              <TimeSeriesChart points={data.timeseries} />
            </ChartHost>
          </div>
        )}
      </InstrumentFrame>
    </div>
  );
}

const manifest: PluginManifest = {
  id: "overview",
  name: "Overview",
  version: "0.3.0",
  required_scopes: ["read:aggregates"],
  requires_query: true,
  nav_items: [{ id: "overview", label: "Overview", path: "overview", order: 10 }],
  routes: [{ id: "overview", path: "overview", component: OverviewPluginView }],
};

export default manifest;
