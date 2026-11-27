import type { KpiSummary } from "@rotameter/plugin-sdk";
import { computeDelta, formatCount, formatDurationMs, formatPercent } from "./format";

export interface KpiStripProps {
  summary: KpiSummary;
  previous?: KpiSummary;
  compare: boolean;
}

interface MetricCardDef {
  key: keyof KpiSummary;
  label: string;
  hint: string;
  format: (val: number) => string;
  invertPolarity?: boolean;
}

const KPI_CARDS: ReadonlyArray<MetricCardDef> = [
  { key: "visitors", label: "VISITORS", hint: "Distinct daily salts", format: formatCount },
  { key: "pageviews", label: "PAGEVIEWS", hint: "Total page hits", format: formatCount },
  { key: "visits", label: "VISITS", hint: "30m sliding sessions", format: formatCount },
  { key: "views_per_visit", label: "VIEWS / VISIT", hint: "Pages per session", format: (v) => v.toFixed(2) },
  { key: "bounce_rate", label: "BOUNCE RATE", hint: "Single-hit sessions", format: formatPercent, invertPolarity: true },
  { key: "avg_visit_duration_ms", label: "AVG DURATION", hint: "Mean session span", format: formatDurationMs },
];

const TONE_CLASS: Record<"ok" | "crit" | "muted", string> = {
  ok: "text-[var(--signal-ok)]",
  crit: "text-[var(--signal-crit)]",
  muted: "text-[var(--fg-muted)]",
};

export function KpiStrip({ summary, previous, compare }: KpiStripProps) {
  return (
    <div className="rm-hairline-grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6">
      {KPI_CARDS.map((card) => {
        const value = summary[card.key];
        const prevVal = previous?.[card.key];
        const delta = compare
          ? computeDelta(value, prevVal, card.invertPolarity)
          : null;
        return (
          <div
            key={card.key}
            className="bg-[var(--bg-surface)] p-[var(--density-panel-pad)] flex flex-col justify-between gap-1"
          >
            <span className="text-[length:var(--text-xs)] uppercase tracking-[0.06em] text-[var(--fg-muted)]">
              {card.label}
            </span>
            <div className="flex items-baseline justify-between gap-2">
              <span className="rm-tabular text-[length:var(--text-kpi)] leading-tight font-semibold text-[var(--fg-primary)]">
                {card.format(value)}
              </span>
              {delta ? (
                <span className={`rm-tabular text-[length:var(--text-xs)] font-medium ${TONE_CLASS[delta.tone]}`}>
                  {delta.label}
                </span>
              ) : null}
            </div>
            <div className="rm-density-expanded rm-tabular text-[length:var(--text-xs)] text-[var(--fg-muted)]">
              {compare && prevVal !== undefined ? `Prev: ${card.format(prevVal)}` : card.hint}
            </div>
          </div>
        );
      })}
    </div>
  );
}
