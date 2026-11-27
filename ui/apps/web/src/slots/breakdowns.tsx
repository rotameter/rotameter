import { useEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  useApiClient,
  useDashboardQuery,
  useSetTelemetryMeta,
  type PluginManifest,
} from "@rotameter/plugin-sdk";
import { InstrumentFrame } from "@rotameter/ui";

function BreakdownsSlot() {
  const api = useApiClient();
  const q = useDashboardQuery();
  const setMeta = useSetTelemetryMeta();

  const { data, isPending, error } = useQuery({
    queryKey: ["breakdowns", q.siteId, q.range, q.filters],
    queryFn: ({ signal }) =>
      api.query(
        {
          site_id: q.siteId,
          range: { preset: q.range },
          metrics: ["visitors", "pageviews", "visits"],
          group_by: ["path"],
          filters: q.filters,
          limit: 20,
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
    <InstrumentFrame title="Breakdowns" subtitle="Top Paths (RM-39 slot)">
      {isPending ? (
        <p className="text-[var(--fg-muted)]">Querying breakdown dimensions…</p>
      ) : error ? (
        <p className="text-[var(--signal-crit)]">{error.message}</p>
      ) : data.rows.length === 0 ? (
        <p className="text-[var(--fg-muted)]">No breakdown rows in this range.</p>
      ) : (
        <ul className="divide-y divide-[var(--border-hairline)]">
          {data.rows.map((row, idx) => (
            <li key={idx} className="flex items-center justify-between py-1.5 text-[var(--text-sm)]">
              <span className="truncate text-[var(--fg-primary)]">{row.key.path ?? "(none)"}</span>
              <span className="rm-tabular text-[var(--fg-secondary)]">{row.visitors} visitors</span>
            </li>
          ))}
        </ul>
      )}
    </InstrumentFrame>
  );
}

const manifest: PluginManifest = {
  id: "breakdowns",
  name: "Breakdowns",
  version: "0.3.0",
  required_scopes: ["read:aggregates"],
  requires_query: true,
  nav_items: [{ id: "breakdowns", label: "Breakdowns", path: "breakdowns", order: 20 }],
  routes: [{ id: "breakdowns", path: "breakdowns", component: BreakdownsSlot }],
};

export default manifest;
