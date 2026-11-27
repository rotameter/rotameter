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

function OverviewSlot() {
  const api = useApiClient();
  const q = useDashboardQuery();
  const setMeta = useSetTelemetryMeta();

  const { data, isPending, error } = useQuery({
    queryKey: ["metrics", q.siteId, q.range, q.interval, q.compare, q.filters],
    queryFn: ({ signal }) =>
      api.query(
        {
          site_id: q.siteId,
          range: { preset: q.range },
          granularity: q.interval,
          metrics: ["visitors", "pageviews", "visits", "views_per_visit", "bounce_rate"],
          compare_previous: q.compare,
          filters: q.filters,
          limit: 10,
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
    <InstrumentFrame
      title="Overview"
      subtitle={`${q.siteId} · ${q.range.toUpperCase()} · ${q.interval}`}
      reticles
    >
      {isPending ? (
        <p className="text-[var(--fg-muted)]">Loading site telemetry…</p>
      ) : error ? (
        <p className="text-[var(--signal-crit)]">{error.message}</p>
      ) : (
        <ChartHost
          vizType="timeseries"
          title="Headline KPIs & Bucket Series"
          fallback={{
            columns: [
              { key: "bucket", label: "Bucket" },
              { key: "visitors", label: "Visitors", align: "right" },
              { key: "pageviews", label: "Pageviews", align: "right" },
              { key: "visits", label: "Visits", align: "right" },
            ],
            rows: data.timeseries,
          }}
        >
          <div className="rm-hairline-grid grid-cols-2 sm:grid-cols-5">
            {[
              ["VISITORS", data.summary.visitors],
              ["PAGEVIEWS", data.summary.pageviews],
              ["VISITS", data.summary.visits],
              ["VIEWS/VISIT", data.summary.views_per_visit.toFixed(2)],
              ["BOUNCE", `${(data.summary.bounce_rate * 100).toFixed(1)}%`],
            ].map(([label, val]) => (
              <div key={String(label)} className="bg-[var(--bg-surface)] p-3">
                <div className="text-[var(--text-xs)] text-[var(--fg-muted)] tracking-[0.06em]">
                  {label}
                </div>
                <div className="rm-tabular text-[var(--text-lg)] font-semibold text-[var(--fg-primary)] mt-1">
                  {val}
                </div>
              </div>
            ))}
          </div>
        </ChartHost>
      )}
    </InstrumentFrame>
  );
}

const manifest: PluginManifest = {
  id: "overview",
  name: "Overview",
  version: "0.3.0",
  required_scopes: ["read:aggregates"],
  requires_query: true,
  nav_items: [{ id: "overview", label: "Overview", path: "overview", order: 10 }],
  routes: [{ id: "overview", path: "overview", component: OverviewSlot }],
};

export default manifest;
