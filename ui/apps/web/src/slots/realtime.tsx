import { useQuery } from "@tanstack/react-query";
import { useApiClient, useDashboardQuery, type PluginManifest } from "@rotameter/plugin-sdk";
import { InstrumentFrame } from "@rotameter/ui";

function RealtimeSlot() {
  const api = useApiClient();
  const q = useDashboardQuery();

  const { data, isPending, error } = useQuery({
    queryKey: ["realtime", q.siteId],
    queryFn: ({ signal }) => api.realtime(q.siteId, signal),
    staleTime: 10_000,
    refetchInterval: 15_000,
    enabled: Boolean(q.siteId),
  });

  return (
    <InstrumentFrame title="Realtime" subtitle="5-minute active window (RM-39 slot)" reticles>
      {isPending ? (
        <p className="text-[var(--fg-muted)]">Polling realtime telemetry…</p>
      ) : error ? (
        <p className="text-[var(--signal-crit)]">{error.message}</p>
      ) : (
        <div className="flex flex-col gap-2">
          <div className="rm-tabular text-[var(--text-xl)] font-semibold text-[var(--signal-ok)]">
            {data.active_visitors} LIVE
          </div>
          <p className="text-[var(--text-xs)] text-[var(--fg-muted)]">
            Active pages: {data.active_pages.length} · 30m pulse samples: {data.pulse.length}
          </p>
        </div>
      )}
    </InstrumentFrame>
  );
}

const manifest: PluginManifest = {
  id: "realtime",
  name: "Realtime",
  version: "0.3.0",
  required_scopes: ["read:aggregates"],
  requires_query: true,
  nav_items: [{ id: "realtime", label: "Realtime", path: "realtime", order: 30 }],
  routes: [{ id: "realtime", path: "realtime", component: RealtimeSlot }],
};

export default manifest;
