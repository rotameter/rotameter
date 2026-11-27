import type { CapabilitiesResponse } from "@rotameter/plugin-sdk";

export type TelemetryTone = "ok" | "warn" | "crit";

export interface FeatureRow {
  key: keyof CapabilitiesResponse["features"];
  label: string;
  enabled: boolean;
}

export function formatUptime(secs: number): string {
  const s = Math.max(0, Math.floor(secs));
  if (s < 60) return `${s}s`;
  const days = Math.floor(s / 86400);
  const hours = Math.floor((s % 86400) / 3600);
  const mins = Math.floor((s % 3600) / 60);
  const rem = s % 60;
  if (days > 0) return `${days}d ${String(hours).padStart(2, "0")}h`;
  if (hours > 0) return `${hours}h ${String(mins).padStart(2, "0")}m`;
  return `${mins}m ${String(rem).padStart(2, "0")}s`;
}

export function queueTone(fillPct: number): TelemetryTone {
  if (fillPct >= 90) return "crit";
  if (fillPct >= 70) return "warn";
  return "ok";
}

export function featureRows(features: CapabilitiesResponse["features"]): FeatureRow[] {
  return [
    { key: "ui", label: "Web UI Serving", enabled: features.ui },
    { key: "arrow_export", label: "Arrow / Parquet Export", enabled: features.arrow_export },
    { key: "oidc", label: "OIDC Auth", enabled: features.oidc },
    { key: "streams", label: "Stream / MCP Hooks", enabled: features.streams },
  ];
}
