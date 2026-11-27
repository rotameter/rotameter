import type {
  DashboardQueryState,
  Granularity,
  QueryFilter,
  RangePreset,
} from "@rotameter/plugin-sdk";

export interface DashboardSearchParams {
  range: RangePreset;
  interval: Granularity;
  compare: boolean;
  f: string;
}

const VALID_RANGES = new Set<RangePreset>(["24h", "7d", "30d", "12m", "all"]);
const VALID_INTERVALS = new Set<Granularity>(["hour", "day", "month"]);

export function defaultIntervalForRange(range: RangePreset): Granularity {
  if (range === "24h") return "hour";
  if (range === "12m" || range === "all") return "month";
  return "day";
}

export function parseFiltersParam(raw: string): QueryFilter[] {
  if (!raw.trim()) return [];
  const out: QueryFilter[] = [];
  for (const part of raw.split(",")) {
    const trimmed = part.trim();
    const colon = trimmed.indexOf(":");
    if (colon <= 0) continue;
    const dimension = trimmed.slice(0, colon).trim();
    const rhs = trimmed.slice(colon + 1).trim();
    if (!dimension || !rhs) continue;
    if (rhs.startsWith("!")) {
      out.push({ dimension, op: "neq", value: rhs.slice(1) });
    } else if (rhs.endsWith("*")) {
      out.push({ dimension, op: "prefix", value: rhs.slice(0, -1) });
    } else if (rhs.startsWith("~")) {
      out.push({ dimension, op: "contains", value: rhs.slice(1) });
    } else if (rhs.includes("|")) {
      out.push({
        dimension,
        op: "in",
        value: rhs
          .split("|")
          .map((v) => v.trim())
          .filter(Boolean),
      });
    } else {
      out.push({ dimension, op: "eq", value: rhs });
    }
  }
  return out;
}

export function serializeFiltersParam(filters: ReadonlyArray<QueryFilter>): string {
  return filters
    .map((f) => {
      if (f.op === "in" && Array.isArray(f.value)) {
        return `${f.dimension}:${f.value.join("|")}`;
      }
      const scalar = Array.isArray(f.value) ? (f.value[0] ?? "") : f.value;
      if (f.op === "neq") return `${f.dimension}:!${scalar}`;
      if (f.op === "prefix") return `${f.dimension}:${scalar}*`;
      if (f.op === "contains") return `${f.dimension}:~${scalar}`;
      return `${f.dimension}:${scalar}`;
    })
    .filter(Boolean)
    .join(",");
}

export function validateDashboardSearch(raw: Record<string, unknown>): DashboardSearchParams {
  const rawRange = typeof raw.range === "string" ? raw.range : "7d";
  const range: RangePreset = VALID_RANGES.has(rawRange as RangePreset)
    ? (rawRange as RangePreset)
    : "7d";
  const rawInterval = typeof raw.interval === "string" ? raw.interval : "";
  const interval: Granularity = VALID_INTERVALS.has(rawInterval as Granularity)
    ? (rawInterval as Granularity)
    : defaultIntervalForRange(range);
  const compare =
    raw.compare === true || raw.compare === "prev" || raw.compare === "true" || raw.compare === "1";
  const f = typeof raw.f === "string" ? raw.f : "";
  return { range, interval, compare, f };
}

export function searchToQueryState(
  siteId: string,
  search: DashboardSearchParams,
): DashboardQueryState {
  return {
    siteId,
    range: search.range,
    interval: search.interval,
    compare: search.compare,
    filters: parseFiltersParam(search.f),
  };
}
