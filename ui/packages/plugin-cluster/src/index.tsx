import { useQuery } from "@tanstack/react-query";
import {
  useApiClient,
  useCapabilities,
  useTelemetryMeta,
  type PluginManifest,
} from "@rotameter/plugin-sdk";
import { BrandIcon, InstrumentFrame } from "@rotameter/ui";
import { featureRows, formatUptime, queueTone, type TelemetryTone } from "./health";

const TONE_TEXT: Record<TelemetryTone, string> = {
  ok: "text-[var(--signal-ok)]",
  warn: "text-[var(--signal-warn)]",
  crit: "text-[var(--signal-crit)]",
};

const TONE_BG: Record<TelemetryTone, string> = {
  ok: "bg-[var(--signal-ok)]",
  warn: "bg-[var(--signal-warn)]",
  crit: "bg-[var(--signal-crit)]",
};

function ClusterPluginView() {
  const api = useApiClient();
  const caps = useCapabilities();
  const meta = useTelemetryMeta();

  const { data, isPending, error } = useQuery({
    queryKey: ["cluster-health"],
    queryFn: ({ signal }) => api.health(signal),
    staleTime: 5_000,
    refetchInterval: 10_000,
  });

  const fillPct = data ? Math.max(0, Math.min(100, data.queue_fill_pct)) : 0;
  const tone = queueTone(fillPct);
  const features = featureRows(caps.features);

  return (
    <div className="flex flex-col gap-[var(--density-grid-gap)]">
      <InstrumentFrame
        title="Node Telemetry"
        subtitle={`${caps.topology.toUpperCase()} · ${caps.node_role}`}
        reticles
      >
        {isPending ? (
          <p className="text-[var(--fg-muted)]">Polling node health…</p>
        ) : error ? (
          <p role="alert" className="text-[var(--signal-crit)]">
            {error.message}
          </p>
        ) : (
          <div className="flex flex-col gap-3">
            <div className="rm-hairline-grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6">
              <div className="bg-[var(--bg-surface)] px-3 py-2.5">
                <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">STATUS</div>
                <div className="rm-tabular text-[var(--text-md)] font-semibold text-[var(--signal-ok)] mt-1">
                  {data.status.toUpperCase()}
                </div>
              </div>
              <div className="bg-[var(--bg-surface)] px-3 py-2.5">
                <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">ROLE</div>
                <div className="rm-tabular text-[var(--text-md)] font-semibold text-[var(--fg-primary)] mt-1">
                  {caps.node_role}
                </div>
              </div>
              <div className="bg-[var(--bg-surface)] px-3 py-2.5">
                <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">VERSION</div>
                <div className="rm-tabular text-[var(--text-md)] font-semibold text-[var(--fg-primary)] mt-1">
                  {data.version}
                </div>
              </div>
              <div className="bg-[var(--bg-surface)] px-3 py-2.5">
                <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">SCHEMA</div>
                <div className="rm-tabular text-[var(--text-md)] font-semibold text-[var(--fg-primary)] mt-1">
                  v{data.schema_version}
                </div>
              </div>
              <div className="bg-[var(--bg-surface)] px-3 py-2.5">
                <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">UPTIME</div>
                <div className="rm-tabular text-[var(--text-md)] font-semibold text-[var(--fg-primary)] mt-1">
                  {formatUptime(data.uptime_secs)}
                </div>
              </div>
              <div className="bg-[var(--bg-surface)] px-3 py-2.5">
                <div className="text-[var(--text-xs)] text-[var(--fg-muted)]">QUEUE FILL</div>
                <div className={`rm-tabular text-[var(--text-md)] font-semibold mt-1 ${TONE_TEXT[tone]}`}>
                  {data.queue_depth} / {data.queue_capacity} ({fillPct.toFixed(1)}%)
                </div>
              </div>
            </div>

            <div className="border border-[var(--border-hairline)] bg-[var(--bg-surface)] px-3 py-2 flex flex-col gap-1.5">
              <div className="flex items-center justify-between text-[var(--text-xs)] text-[var(--fg-muted)]">
                <span className="inline-flex items-center gap-1.5">
                  <BrandIcon name="rm-threshold" className="text-[var(--fg-muted)]" />
                  <span>INGEST RING BUFFER SATURATION</span>
                </span>
                <span className={`rm-tabular font-semibold ${TONE_TEXT[tone]}`}>
                  {fillPct.toFixed(1)}%
                </span>
              </div>
              <div
                role="progressbar"
                aria-label="Ingest queue fill percentage"
                aria-valuemin={0}
                aria-valuemax={100}
                aria-valuenow={Math.round(fillPct)}
                className="h-2 w-full bg-[var(--bg-sunken)] border border-[var(--border-hairline)] overflow-hidden"
              >
                <div
                  className={`h-full ${TONE_BG[tone]}`}
                  style={{ width: `${fillPct}%` }}
                />
              </div>
            </div>
          </div>
        )}
      </InstrumentFrame>

      <InstrumentFrame
        title="Capabilities & Topology"
        subtitle={`Topology: ${caps.topology}`}
      >
        <div className="flex flex-col gap-3">
          {caps.topology === "cluster" ? (
            <div className="border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-2.5 py-1.5 text-[var(--text-xs)] text-[var(--fg-secondary)]">
              Multi-node roster grid arrives in 0.5.0 (RM-22).
            </div>
          ) : null}

          <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-2">
            {features.map((feat) => (
              <div
                key={feat.key}
                className="flex items-center justify-between border border-[var(--border-hairline)] bg-[var(--bg-surface)] px-2.5 py-2 text-[var(--text-sm)]"
              >
                <span className="text-[var(--fg-secondary)]">{feat.label}</span>
                <span
                  className={`rm-tabular text-[var(--text-xs)] font-semibold ${
                    feat.enabled ? "text-[var(--signal-ok)]" : "text-[var(--fg-muted)]"
                  }`}
                >
                  {feat.enabled ? "ON" : "OFF"}
                </span>
              </div>
            ))}
          </div>

          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2 text-[var(--text-xs)]">
            <div className="border border-[var(--border-hairline)] bg-[var(--bg-sunken)] px-2.5 py-2">
              <div className="text-[var(--fg-muted)] uppercase tracking-[0.05em] mb-1">
                Auth Methods & Endpoints
              </div>
              <div className="rm-tabular text-[var(--fg-primary)]">
                auth: {caps.auth_methods.join(", ") || "none"}
              </div>
              <div className="rm-tabular text-[var(--fg-secondary)]">
                query: {caps.endpoints.query} · stream: {caps.endpoints.stream}
              </div>
            </div>

            {meta && (meta.served_by_node || meta.watermark_ts) ? (
              <div className="border border-[var(--border-hairline)] bg-[var(--bg-sunken)] px-2.5 py-2">
                <div className="text-[var(--fg-muted)] uppercase tracking-[0.05em] mb-1">
                  Last Query Routing Metadata
                </div>
                {meta.served_by_node ? (
                  <div className="rm-tabular text-[var(--fg-primary)]">
                    served_by_node: {meta.served_by_node}
                  </div>
                ) : null}
                {meta.watermark_ts ? (
                  <div className="rm-tabular text-[var(--fg-secondary)]">
                    watermark_ts: {meta.watermark_ts}
                  </div>
                ) : null}
              </div>
            ) : null}
          </div>
        </div>
      </InstrumentFrame>
    </div>
  );
}

const manifest: PluginManifest = {
  id: "cluster",
  name: "Cluster",
  version: "0.3.0",
  required_scopes: [],
  nav_items: [{ id: "cluster", label: "Cluster", path: "/cluster", order: 90 }],
  routes: [{ id: "cluster", path: "/cluster", component: ClusterPluginView }],
};

export default manifest;
