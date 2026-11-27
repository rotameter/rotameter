import { useEffect, useMemo, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  useApiClient,
  useAuth,
  useCapabilities,
  useDashboardQuery,
  useSetTelemetryMeta,
  useUpdateQuery,
  type PluginManifest,
} from "@rotameter/plugin-sdk";
import { BrandIcon, Button, InstrumentFrame, Select, Tabs } from "@rotameter/ui";
import { BreakdownTable, type BreakdownRowItem } from "./BreakdownTable";
import {
  BREAKDOWN_GROUPS,
  toggleDimensionFilter,
  type BreakdownGroupId,
} from "./dimensions";

function triggerDownload(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

function BreakdownsPluginView() {
  const api = useApiClient();
  const auth = useAuth();
  const caps = useCapabilities();
  const q = useDashboardQuery();
  const updateQuery = useUpdateQuery();
  const setMeta = useSetTelemetryMeta();

  const [groupId, setGroupId] = useState<BreakdownGroupId>("pages");
  const [dimension, setDimension] = useState<string>("path");
  const [exporting, setExporting] = useState<"csv" | "parquet" | null>(null);
  const [exportError, setExportError] = useState<string | null>(null);

  const activeGroup = useMemo(
    () => BREAKDOWN_GROUPS.find((g) => g.id === groupId) ?? BREAKDOWN_GROUPS[0]!,
    [groupId],
  );
  const activeDimLabel =
    activeGroup.dimensions.find((d) => d.value === dimension)?.label ?? dimension;

  const { data, isPending, error } = useQuery({
    queryKey: ["breakdowns", q.siteId, q.range, q.filters, dimension],
    queryFn: ({ signal }) =>
      api.query(
        {
          site_id: q.siteId,
          range: { preset: q.range },
          metrics: ["visitors", "pageviews", "visits", "bounce_rate", "avg_visit_duration_ms"],
          group_by: [dimension],
          filters: q.filters,
          limit: 200,
        },
        signal,
      ),
    staleTime: 30_000,
    enabled: Boolean(q.siteId),
  });

  useEffect(() => {
    if (data?.meta) setMeta(data.meta);
  }, [data?.meta, setMeta]);

  const rows: BreakdownRowItem[] = useMemo(
    () =>
      (data?.rows ?? []).map((r) => ({
        value: r.key[dimension] || "(none)",
        visitors: r.visitors,
        pageviews: r.pageviews,
        visits: r.visits,
        bounce_rate: r.bounce_rate,
        avg_visit_duration_ms: r.avg_visit_duration_ms,
      })),
    [data?.rows, dimension],
  );

  const activeFilterValue = useMemo(() => {
    const match = q.filters.find((f) => f.dimension === dimension && f.op === "eq");
    return typeof match?.value === "string" ? match.value : null;
  }, [q.filters, dimension]);

  const handleExportCsv = async () => {
    setExporting("csv");
    setExportError(null);
    try {
      const csv = await api.queryCsv({
        site_id: q.siteId,
        range: { preset: q.range },
        metrics: ["visitors", "pageviews", "visits", "bounce_rate", "avg_visit_duration_ms"],
        group_by: [dimension],
        filters: q.filters,
        limit: 1000,
      });
      triggerDownload(
        new Blob([csv], { type: "text/csv;charset=utf-8" }),
        `${q.siteId}-${dimension}-${q.range}.csv`,
      );
    } catch (err) {
      setExportError(err instanceof Error ? err.message : "CSV export failed");
    } finally {
      setExporting(null);
    }
  };

  const handleExportParquet = async () => {
    setExporting("parquet");
    setExportError(null);
    try {
      const archive = await api.exportPortableArchive();
      triggerDownload(archive, `${q.siteId}-parquet-backup.tar.zst`);
    } catch (err) {
      setExportError(err instanceof Error ? err.message : "Parquet archive export failed");
    } finally {
      setExporting(null);
    }
  };

  const canParquetExport = caps.features.arrow_export && auth.scope === "admin";

  const tableContent = isPending ? (
    <p className="text-[var(--fg-muted)]">Querying {activeDimLabel} breakdown…</p>
  ) : error ? (
    <p className="text-[var(--signal-crit)]">{error.message}</p>
  ) : (
    <div className="flex flex-col gap-2">
      {data.meta.truncated_by_cardinality_cap ? (
        <div className="border border-[var(--signal-warn)] bg-[var(--bg-elevated)] px-2.5 py-1.5 text-[length:var(--text-xs)] text-[var(--signal-warn)]">
          Breakdown truncated at server cardinality cap ({data.meta.total_rows} rows shown).
        </div>
      ) : null}
      <BreakdownTable
        dimensionLabel={activeDimLabel}
        rows={rows}
        activeFilterValue={activeFilterValue}
        onSelectValue={(val) =>
          updateQuery(
            { filters: toggleDimensionFilter(q.filters, dimension, val) },
            { mode: "push" },
          )
        }
      />
    </div>
  );

  return (
    <InstrumentFrame
      title="Dimension Breakdowns"
      subtitle={`${q.siteId} · ${activeDimLabel} (${rows.length} rows)`}
      reticles
      actions={
        <div className="flex flex-wrap items-center gap-2">
          <Select
            ariaLabel="Breakdown dimension"
            value={dimension}
            onValueChange={setDimension}
            options={activeGroup.dimensions}
          />
          <Button
            variant="secondary"
            disabled={exporting !== null}
            onClick={() => void handleExportCsv()}
          >
            <BrandIcon name="rm-export" />
            <span>{exporting === "csv" ? "Exporting…" : "Export CSV"}</span>
          </Button>
          <Button
            variant="secondary"
            disabled={!canParquetExport || exporting !== null}
            title={
              !caps.features.arrow_export
                ? "Requires server built with `--features arrow`"
                : auth.scope !== "admin"
                  ? "Requires admin scope for full Parquet archive"
                  : "Export portable Arrow/Parquet .tar.zst archive"
            }
            onClick={() => void handleExportParquet()}
          >
            <BrandIcon name="rm-export" />
            <span>{exporting === "parquet" ? "Packing…" : "Export Parquet"}</span>
          </Button>
        </div>
      }
    >
      <div className="flex flex-col gap-2">
        {q.filters.length > 0 ? (
          <div className="flex flex-wrap items-center gap-1.5" aria-label="Active filters">
            {q.filters.map((f, idx) => (
              <Button
                key={`${f.dimension}-${idx}`}
                variant="secondary"
                onClick={() =>
                  updateQuery(
                    { filters: q.filters.filter((_, i) => i !== idx) },
                    { mode: "push" },
                  )
                }
              >
                <span className="rm-tabular">
                  {f.dimension}:{Array.isArray(f.value) ? f.value.join("|") : f.value} ×
                </span>
              </Button>
            ))}
          </div>
        ) : null}
        {exportError ? (
          <p className="text-[var(--signal-crit)] text-[length:var(--text-sm)]">{exportError}</p>
        ) : null}
        <Tabs
          value={groupId}
          onValueChange={(nextId) => {
            const id = nextId as BreakdownGroupId;
            setGroupId(id);
            const grp = BREAKDOWN_GROUPS.find((g) => g.id === id);
            const firstDim = grp?.dimensions[0]?.value;
            if (firstDim) setDimension(firstDim);
          }}
          items={BREAKDOWN_GROUPS.map((g) => ({
            value: g.id,
            label: g.label,
            content: g.id === groupId ? tableContent : null,
          }))}
        />
      </div>
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
  routes: [{ id: "breakdowns", path: "breakdowns", component: BreakdownsPluginView }],
};

export default manifest;
