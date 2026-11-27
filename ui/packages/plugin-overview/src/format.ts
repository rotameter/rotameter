import type { QueryResponse } from "@rotameter/plugin-sdk";

export interface KpiDelta {
  pct: number;
  label: string;
  tone: "ok" | "crit" | "muted";
}

export function formatCount(n: number): string {
  return Math.round(n).toLocaleString("en-US");
}

export function formatPercent(ratio: number): string {
  return `${(ratio * 100).toFixed(1)}%`;
}

export function formatDurationMs(ms: number): string {
  const totalSecs = Math.max(0, Math.round(ms / 1000));
  if (totalSecs < 60) return `${totalSecs}s`;
  const hours = Math.floor(totalSecs / 3600);
  const mins = Math.floor((totalSecs % 3600) / 60);
  const secs = totalSecs % 60;
  if (hours > 0) {
    return `${hours}h ${String(mins).padStart(2, "0")}m`;
  }
  return `${mins}m ${String(secs).padStart(2, "0")}s`;
}

export function computeDelta(
  current: number,
  previous: number | undefined,
  invertPolarity = false,
): KpiDelta | null {
  if (previous === undefined || previous === null) return null;
  if (previous === 0) {
    if (current === 0) return { pct: 0, label: "0.0%", tone: "muted" };
    return {
      pct: 100,
      label: "+100.0%",
      tone: invertPolarity ? "crit" : "ok",
    };
  }
  const pct = ((current - previous) / previous) * 100;
  if (Math.abs(pct) < 0.05) {
    return { pct: 0, label: "0.0%", tone: "muted" };
  }
  const positive = pct > 0;
  const good = invertPolarity ? !positive : positive;
  return {
    pct,
    label: `${positive ? "+" : ""}${pct.toFixed(1)}%`,
    tone: good ? "ok" : "crit",
  };
}

export function buildUplotData(
  points: QueryResponse["timeseries"],
): [number[], number[], number[], number[]] {
  const xs: number[] = [];
  const visitors: number[] = [];
  const pageviews: number[] = [];
  const visits: number[] = [];
  for (const pt of points) {
    const ms = Date.parse(pt.bucket);
    xs.push(Number.isFinite(ms) ? Math.floor(ms / 1000) : xs.length);
    visitors.push(pt.visitors);
    pageviews.push(pt.pageviews);
    visits.push(pt.visits);
  }
  return [xs, visitors, pageviews, visits];
}
