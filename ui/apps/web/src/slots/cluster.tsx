import { useQuery } from "@tanstack/react-query";
import { useApiClient, useCapabilities, type PluginManifest } from "@rotameter/plugin-sdk";
import { InstrumentFrame } from "@rotameter/ui";

function ClusterSlot() {
  const api = useApiClient();
  const caps = useCapabilities();
  const { data } = useQuery({
    queryKey: ["health"],
    queryFn: ({ signal }) => api.health(signal),
    staleTime: 10_000,
  });

  return (
    <InstrumentFrame title="Node Telemetry" subtitle={`${caps.topology.toUpperCase()} · ${caps.node_role}`}>
      <div className="rm-tabular grid grid-cols-2 sm:grid-cols-4 gap-3 text-[var(--text-sm)]">
        <div>
          <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">STATUS</div>
          <div className="text-[var(--signal-ok)] font-semibold">{data?.status ?? "ok"}</div>
        </div>
        <div>
          <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">ROLE</div>
          <div>{caps.node_role}</div>
        </div>
        <div>
          <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">QUEUE</div>
          <div>
            {data ? `${data.queue_depth}/${data.queue_capacity} (${data.queue_fill_pct.toFixed(1)}%)` : "—"}
          </div>
        </div>
        <div>
          <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">SCHEMA</div>
          <div>v{data?.schema_version ?? "—"}</div>
        </div>
      </div>
    </InstrumentFrame>
  );
}

const manifest: PluginManifest = {
  id: "cluster",
  name: "Cluster",
  version: "0.3.0",
  required_scopes: [],
  nav_items: [{ id: "cluster", label: "Cluster", path: "/cluster", order: 90 }],
  routes: [{ id: "cluster", path: "/cluster", component: ClusterSlot }],
};

export default manifest;
