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
  format: (val: number) => string;
  invertPolarity?: boolean;
}

const KPI_CARDS: ReadonlyArray<MetricCardDef> = [
  { key: "visitors", label: "VISITORS", format: formatCount },
  { key: "pageviews", label: "PAGEVIEWS", format: formatCount },
  { key: "visits", label: "VISITS", format: formatCount },
  { key: "views_per_visit", label: "VIEWS / VISIT", format: (v) => v.toFixed(2) },
  { key: "bounce_rate", label: "BOUNCE RATE", format: formatPercent, invertPolarity: true },
  { key: "avg_visit_duration_ms", label: "AVG DURATION", format: formatDurationMs },
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
        const delta = compare
          ? computeDelta(value, previous?.[card.key], card.invertPolarity)
          : null;
        return (
          <div
            key={card.key}
            className="bg-[var(--bg-surface)] px-3 py-2.5 flex flex-col justify-between gap-1"
          >
            <span className="text-[var(--text-xs)] uppercase tracking-[0.06em] text-[var(--fg-muted)]">
              {card.label}
            </span>
            <div className="flex items-baseline justify-between gap-2">
              <span className="rm-tabular text-[var(--text-lg)] font-semibold text-[var(--fg-primary)]">
                {card.format(value)}
              </span>
              {delta ? (
                <span className={`rm-tabular text-[var(--text-xs)] font-medium ${TONE_CLASS[delta.tone]}`}>
                  {delta.label}
                </span>
              ) : null}
            </div>
          </div>
        );
      })}
    </div>
  );
}
