import { useQuery } from "@tanstack/react-query";
import type { CapabilitiesResponse, RotameterApiClient } from "@rotameter/plugin-sdk";
import { BrandIcon, InstrumentFrame } from "@rotameter/ui";

export interface IngestRoleViewProps {
  api: RotameterApiClient;
  capabilities: CapabilitiesResponse;
}

export function IngestRoleView({ api, capabilities }: IngestRoleViewProps) {
  const { data } = useQuery({
    queryKey: ["ingest-health"],
    queryFn: ({ signal }) => api.health(signal),
    refetchInterval: 10_000,
  });

  return (
    <main className="min-h-screen p-6 bg-[var(--bg-canvas)]">
      <div className="max-w-3xl mx-auto flex flex-col gap-4">
        <div className="flex items-center gap-2">
          <BrandIcon name="rm-logo" className="w-5 h-5 text-[var(--accent-primary)]" />
          <span className="font-semibold tracking-[0.1em] text-[var(--text-sm)] text-[var(--fg-primary)]">
            ROTAMETER
          </span>
        </div>
        <InstrumentFrame
          title="NODE ROLE TELEMETRY // INGEST-ONLY"
          subtitle={`${capabilities.topology.toUpperCase()} · ${capabilities.node_role}`}
          reticles
        >
          <p className="text-[var(--text-sm)] text-[var(--fg-secondary)] mb-4">
            This node runs with <code>--role=ingest</code>. Query and admin UI routes are gated off;
            point your dashboard at a <code>query</code> or <code>all</code> node to inspect metrics.
          </p>
          <div className="rm-hairline-grid grid-cols-2 sm:grid-cols-4">
            <div className="bg-[var(--bg-surface)] p-3">
              <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">STATUS</div>
              <div className="rm-tabular text-[var(--text-md)] font-semibold text-[var(--signal-ok)]">
                {data?.status ?? "ok"}
              </div>
            </div>
            <div className="bg-[var(--bg-surface)] p-3">
              <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">QUEUE DEPTH</div>
              <div className="rm-tabular text-[var(--text-md)] font-semibold">
                {data ? `${data.queue_depth} / ${data.queue_capacity}` : "—"}
              </div>
            </div>
            <div className="bg-[var(--bg-surface)] p-3">
              <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">QUEUE FILL</div>
              <div className="rm-tabular text-[var(--text-md)] font-semibold">
                {data ? `${data.queue_fill_pct.toFixed(1)}%` : "—"}
              </div>
            </div>
            <div className="bg-[var(--bg-surface)] p-3">
              <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">UPTIME</div>
              <div className="rm-tabular text-[var(--text-md)] font-semibold">
                {data ? `${data.uptime_secs}s` : "—"}
              </div>
            </div>
          </div>
        </InstrumentFrame>
      </div>
    </main>
  );
}
