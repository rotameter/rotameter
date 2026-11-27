import type { ComponentType, ReactNode } from "react";

export type NodeRole = "all" | "ingest" | "query";

export interface CapabilitiesResponse {
  topology: "single" | "cluster";
  node_role: NodeRole;
  features: {
    ui: boolean;
    arrow_export: boolean;
    oidc: boolean;
    streams: boolean;
  };
  auth_methods: string[];
  endpoints: {
    query: string;
    stream: string;
  };
}

export interface AuthMeResponse {
  authenticated: boolean;
  method: "session" | "oidc" | "bearer" | "cf_access";
  subject: string;
  scope: "read" | "admin" | "ingest";
  org_id: string;
  site_id?: string;
}

export interface SiteResponse {
  site_id: string;
  org_id: string;
  slug: string;
  display_name: string;
  timezone: string;
  salt_rotation: string;
  respect_gpc: boolean;
  is_self_site: boolean;
  allowed_origins: string[];
  allowed_query_keys: string[];
  raw_retention_days: number | null;
  created_at: string;
  updated_at: string;
}

export interface QueryMeta {
  total_rows: number;
  query_ms: number;
  truncated_by_cardinality_cap: boolean;
  watermark_ts?: string;
  partial_nodes?: number;
  served_by_node?: string;
}

export interface KpiSummary {
  visitors: number;
  pageviews: number;
  visits: number;
  views_per_visit: number;
  bounce_rate: number;
  avg_visit_duration_ms: number;
}

export interface QueryFilter {
  dimension: string;
  op: "eq" | "neq" | "in" | "contains" | "prefix";
  value: string | string[];
}

export type RangePreset = "24h" | "7d" | "30d" | "12m" | "all";
export type Granularity = "hour" | "day" | "month";

export interface QueryRequest {
  site_id: string;
  range: { preset: RangePreset } | { from: string; to: string };
  granularity?: Granularity;
  metrics: string[];
  group_by?: string[];
  filters?: QueryFilter[];
  compare_previous?: boolean;
  limit?: number;
  offset?: number;
  format?: "json" | "csv";
}

export interface QueryResponse {
  site_id: string;
  slug: string;
  timezone: string;
  window: { from: string; to: string };
  summary: KpiSummary;
  previous_summary?: KpiSummary;
  timeseries: Array<{ bucket: string; visitors: number; pageviews: number; visits: number }>;
  rows: Array<{
    key: Record<string, string>;
    visitors: number;
    pageviews: number;
    visits: number;
    bounce_rate: number;
    avg_visit_duration_ms: number;
  }>;
  meta: QueryMeta;
}

export interface RealtimeResponse {
  site_id: string;
  slug: string;
  active_visitors: number;
  active_pages: Array<{ path: string; active_visitors: number }>;
  pulse: Array<{ minute: string; events: number }>;
}

export interface HealthResponse {
  status: string;
  role: NodeRole;
  version: string;
  schema_version: number;
  queue_depth: number;
  queue_capacity: number;
  queue_fill_pct: number;
  uptime_secs: number;
}

export interface DashboardQueryState {
  siteId: string;
  range: RangePreset;
  interval: Granularity;
  compare: boolean;
  filters: QueryFilter[];
}

export type ApiScope =
  | "read:aggregates"
  | "read:events"
  | "write:dashboards"
  | "admin:sites"
  | "admin:keys";

export interface PluginNavItem {
  id: string;
  label: string;
  path: string;
  order?: number;
}

export interface PluginRoute {
  id: string;
  path: string;
  component: ComponentType;
}

export interface PluginPanel {
  id: string;
  title: string;
  component: ComponentType;
}

export interface PluginVisualization {
  viz_type: string;
  label: string;
  render: ComponentType<{ data: unknown }>;
}

export interface PluginInspectorTab {
  id: string;
  label: string;
  render: ComponentType<{ row: Record<string, unknown> }>;
}

export interface PluginRowAction {
  id: string;
  label: string;
  onAction: (row: Record<string, unknown>) => void;
}

export interface PluginQueryTransform {
  id: string;
  transform: (req: QueryRequest) => QueryRequest;
}

export interface PluginExporter {
  id: string;
  label: string;
  exportData: (res: QueryResponse) => Promise<void> | void;
}

export interface PluginManifest {
  id: string;
  name: string;
  version: string;
  required_scopes: ApiScope[];
  requires_query?: boolean;
  routes?: PluginRoute[];
  nav_items?: PluginNavItem[];
  dashboard_panels?: PluginPanel[];
  visualizations?: PluginVisualization[];
  inspector_tabs?: PluginInspectorTab[];
  row_actions?: PluginRowAction[];
  query_transforms?: PluginQueryTransform[];
  exporters?: PluginExporter[];
}

export type LazyPluginLoader = () => Promise<{ default: PluginManifest }>;

export interface TabularFallbackColumn {
  key: string;
  label: string;
  align?: "left" | "right";
}

export interface ChartHostFallbackData {
  columns: TabularFallbackColumn[];
  rows: Array<Record<string, string | number>>;
  caption?: string;
}

export interface TelemetryRenderContext {
  children?: ReactNode;
}
