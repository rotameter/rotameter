import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  ChartHost,
  useApiClient,
  useDashboardQuery,
  useUpdateQuery,
  type PluginManifest,
} from "@rotameter/plugin-sdk";
import { Button, InstrumentFrame } from "@rotameter/ui";
import { PulseChart } from "./PulseChart";

function RealtimePluginView() {
  const api = useApiClient();
  const q = useDashboardQuery();
  const updateQuery = useUpdateQuery();
  const [paused, setPaused] = useState(false);

  const { data, isPending, error, dataUpdatedAt } = useQuery({
    queryKey: ["realtime", q.siteId],
    queryFn: ({ signal }) => api.realtime(q.siteId, signal),
    staleTime: 5_000,
    refetchInterval: paused ? false : 10_000,
    enabled: Boolean(q.siteId),
  });

  const maxActive = data?.active_pages.reduce((acc, p) => Math.max(acc, p.active_visitors), 0) ?? 0;

  return (
    <div className="flex flex-col gap-[var(--density-grid-gap)]">
      <InstrumentFrame
        title="Realtime Pulse"
        subtitle={`${q.siteId} · trailing 5m active / 30m stream`}
        reticles
        actions={
          <Button
            variant={paused ? "secondary" : "primary"}
            aria-pressed={paused}
            onClick={() => setPaused((p) => !p)}
          >
            {paused ? "Resume Poll" : "Pause Poll"}
          </Button>
        }
      >
        {isPending ? (
          <p className="text-[var(--fg-muted)]">Polling realtime telemetry…</p>
        ) : error ? (
          <p className="text-[var(--signal-crit)]">{error.message}</p>
        ) : (
          <div className="flex flex-col gap-[var(--density-grid-gap)]">
            <div className="flex flex-wrap items-center justify-between gap-3 border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-3 py-2.5">
              <div className="flex items-center gap-3">
                <span
                  aria-hidden
                  className="inline-block w-2.5 h-2.5 rounded-full bg-[var(--signal-ok)]"
                />
                <div>
                  <div className="text-[var(--text-xs)] uppercase tracking-[0.06em] text-[var(--fg-muted)]">
                    Active Visitors (5m)
                  </div>
                  <div className="rm-tabular text-[length:var(--density-kpi)] leading-tight font-semibold text-[var(--fg-primary)]">
                    {data.active_visitors.toLocaleString("en-US")}
                  </div>
                </div>
              </div>
              <div className="rm-tabular text-[var(--text-xs)] text-[var(--fg-muted)]">
                {paused ? "PAUSED" : "POLLING 10s"} · Updated{" "}
                {dataUpdatedAt ? new Date(dataUpdatedAt).toLocaleTimeString() : "--"}
              </div>
            </div>

            <ChartHost
              vizType="realtime-pulse"
              title="30-Minute Minute Pulse"
              fallback={{
                caption: `30-minute event pulse for ${q.siteId}`,
                columns: [
                  { key: "minute", label: "Minute (UTC)" },
                  { key: "events", label: "Events", align: "right" },
                ],
                rows: data.pulse,
              }}
            >
              <PulseChart pulse={data.pulse} />
            </ChartHost>

            <div className="border border-[var(--border-hairline)] bg-[var(--bg-surface)]">
              <div className="border-b border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-2.5 py-1.5 flex items-center justify-between text-[var(--text-xs)] uppercase tracking-[0.05em] text-[var(--fg-secondary)]">
                <span>Active Pages (Click to Filter Breakdowns)</span>
                <span className="rm-tabular">Visitors</span>
              </div>
              {data.active_pages.length === 0 ? (
                <p className="px-2.5 py-4 text-center text-[var(--text-sm)] text-[var(--fg-muted)]">
                  No active pages in trailing 5 minutes.
                </p>
              ) : (
                <ul className="divide-y divide-[var(--border-hairline)]">
                  {data.active_pages.map((page) => {
                    const widthPct =
                      maxActive > 0 ? Math.round((page.active_visitors / maxActive) * 100) : 0;
                    return (
                      <li key={page.path}>
                        <button
                          type="button"
                          onClick={() =>
                            updateQuery(
                              {
                                filters: [{ dimension: "path", op: "eq", value: page.path }],
                              },
                              { mode: "push" },
                            )
                          }
                          className="relative w-full h-[var(--density-table-row-h)] px-2.5 flex items-center justify-between text-left hover:bg-[var(--bg-elevated)] cursor-pointer"
                        >
                          <span
                            aria-hidden
                            className="absolute inset-y-0.5 left-0 bg-[var(--series-1)]/15 pointer-events-none"
                            style={{ width: `${widthPct}%` }}
                          />
                          <span className="relative truncate text-[var(--fg-primary)]">
                            {page.path}
                          </span>
                          <span className="relative rm-tabular text-[var(--fg-secondary)] font-medium">
                            {page.active_visitors.toLocaleString("en-US")}
                          </span>
                        </button>
                      </li>
                    );
                  })}
                </ul>
              )}
            </div>
          </div>
        )}
      </InstrumentFrame>
    </div>
  );
}

const manifest: PluginManifest = {
  id: "realtime",
  name: "Realtime",
  version: "0.3.0",
  required_scopes: ["read:aggregates"],
  requires_query: true,
  nav_items: [{ id: "realtime", label: "Realtime", path: "realtime", order: 30 }],
  routes: [{ id: "realtime", path: "realtime", component: RealtimePluginView }],
};

export default manifest;
