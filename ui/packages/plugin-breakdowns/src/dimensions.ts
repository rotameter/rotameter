import type { QueryFilter } from "@rotameter/plugin-sdk";

export type BreakdownGroupId = "pages" | "referrers" | "utm" | "devices_geo";

export interface BreakdownDimensionOption {
  value: string;
  label: string;
}

export interface BreakdownGroupDef {
  id: BreakdownGroupId;
  label: string;
  dimensions: ReadonlyArray<BreakdownDimensionOption>;
}

export const BREAKDOWN_GROUPS: ReadonlyArray<BreakdownGroupDef> = [
  {
    id: "pages",
    label: "Pages",
    dimensions: [
      { value: "path", label: "Page Path" },
      { value: "entry_page", label: "Entry Page" },
      { value: "exit_page", label: "Exit Page" },
    ],
  },
  {
    id: "referrers",
    label: "Referrers",
    dimensions: [
      { value: "source", label: "Source" },
      { value: "channel", label: "Channel" },
      { value: "referrer", label: "Referrer URL" },
    ],
  },
  {
    id: "utm",
    label: "UTM / Campaigns",
    dimensions: [
      { value: "utm_source", label: "UTM Source" },
      { value: "utm_medium", label: "UTM Medium" },
      { value: "utm_campaign", label: "UTM Campaign" },
    ],
  },
  {
    id: "devices_geo",
    label: "Devices & Geo",
    dimensions: [
      { value: "country", label: "Country" },
      { value: "region", label: "Region" },
      { value: "browser", label: "Browser" },
      { value: "os", label: "OS" },
      { value: "device", label: "Device" },
      { value: "viewport", label: "Viewport" },
    ],
  },
];

export function parsePxValue(raw: string, fallback = 34): number {
  const num = Number.parseFloat(raw.trim());
  return Number.isFinite(num) && num > 0 ? num : fallback;
}

export function computeBarRatio(value: number, max: number): number {
  if (!Number.isFinite(value) || !Number.isFinite(max) || max <= 0) return 0;
  return Math.max(0, Math.min(1, value / max));
}

export function toggleDimensionFilter(
  filters: ReadonlyArray<QueryFilter>,
  dimension: string,
  value: string,
): QueryFilter[] {
  const exists = filters.some(
    (f) => f.dimension === dimension && f.op === "eq" && f.value === value,
  );
  if (exists) {
    return filters.filter(
      (f) => !(f.dimension === dimension && f.op === "eq" && f.value === value),
    );
  }
  const rest = filters.filter((f) => !(f.dimension === dimension && f.op === "eq"));
  return [...rest, { dimension, op: "eq", value }];
}

export function formatDurationSeconds(ms: number): string {
  const secs = Math.max(0, Math.round(ms / 1000));
  if (secs < 60) return `${secs}s`;
  const mins = Math.floor(secs / 60);
  const rem = secs % 60;
  return `${mins}m ${String(rem).padStart(2, "0")}s`;
}
