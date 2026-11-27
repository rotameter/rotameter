import { useState, type ReactNode } from "react";
import {
  useAuth,
  useCapabilities,
  useDashboardQuery,
  useTelemetryMeta,
  useUpdateQuery,
  type Granularity,
  type PluginManifest,
  type RangePreset,
  type SiteResponse,
} from "@rotameter/plugin-sdk";
import {
  BrandIcon,
  Button,
  Input,
  Select,
  type BrandIconName,
} from "@rotameter/ui";
import { serializeFiltersParam, parseFiltersParam } from "../searchParams";
import type { DensityPreference, ThemePreference, UiPreferences } from "../preferences";

const NAV_ICON_BY_ID: Record<string, BrandIconName> = {
  overview: "rm-throughput",
  breakdowns: "rm-segments",
  realtime: "rm-stream",
  admin: "rm-control",
  cluster: "rm-equilibrium",
};

export interface AppShellProps {
  plugins: PluginManifest[];
  sites: SiteResponse[];
  activeRouteId: string;
  onNavigatePlugin: (path: string) => void;
  onSelectSite: (siteId: string) => void;
  prefs: UiPreferences;
  onChangePrefs: (next: UiPreferences) => void;
  onLogout: () => void;
  children: ReactNode;
}

const RANGE_OPTIONS: Array<{ value: RangePreset; label: string }> = [
  { value: "24h", label: "24H" },
  { value: "7d", label: "7D" },
  { value: "30d", label: "30D" },
  { value: "12m", label: "12M" },
  { value: "all", label: "ALL" },
];

const INTERVAL_OPTIONS: Array<{ value: Granularity; label: string }> = [
  { value: "hour", label: "1h" },
  { value: "day", label: "1d" },
  { value: "month", label: "1m" },
];

export function AppShell({
  plugins,
  sites,
  activeRouteId,
  onNavigatePlugin,
  onSelectSite,
  prefs,
  onChangePrefs,
  onLogout,
  children,
}: AppShellProps) {
  const caps = useCapabilities();
  const auth = useAuth();
  const q = useDashboardQuery();
  const updateQuery = useUpdateQuery();
  const meta = useTelemetryMeta();
  const [filterInput, setFilterInput] = useState(() => serializeFiltersParam(q.filters));

  const navItems = plugins
    .flatMap((p) => p.nav_items ?? [])
    .sort((a, b) => (a.order ?? 100) - (b.order ?? 100));

  const telemetryLabel = meta?.partial_nodes
    ? `CLUSTER · PARTIAL (${meta.partial_nodes})`
    : `${caps.topology.toUpperCase()}-NODE · ${meta ? `${meta.query_ms.toFixed(1)}ms` : caps.node_role}`;

  return (
    <div className="min-h-screen flex flex-col bg-[var(--bg-canvas)] text-[var(--fg-primary)]">
      <header className="border-b border-[var(--border-hairline)] bg-[var(--bg-surface)] px-3 py-2 flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-center gap-2">
          <Button
            variant="ghost"
            aria-label={prefs.sidebarCollapsed ? "Expand sidebar" : "Collapse sidebar"}
            onClick={() => onChangePrefs({ ...prefs, sidebarCollapsed: !prefs.sidebarCollapsed })}
          >
            ☰
          </Button>
          <span className="inline-flex items-center gap-1.5 font-semibold tracking-[0.08em] text-[var(--text-sm)]">
            <BrandIcon name="rm-logo" className="w-5 h-5 text-[var(--accent-primary)]" />
            <span>ROTAMETER</span>
          </span>
          {auth.scope === "admin" && sites.length > 0 ? (
            <Select
              ariaLabel="Active site"
              value={q.siteId}
              onValueChange={onSelectSite}
              options={sites.map((s) => ({ value: s.slug, label: s.slug }))}
            />
          ) : (
            <span className="rm-tabular border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-2 py-1 text-[var(--text-xs)] rounded-[var(--radius-xs)]">
              {q.siteId}
            </span>
          )}
          <div className="flex items-center gap-1.5">
            <BrandIcon name="rm-range" className="text-[var(--fg-muted)]" />
            <Select
              ariaLabel="Time range"
              value={q.range}
              onValueChange={(range) => updateQuery({ range }, { mode: "push" })}
              options={RANGE_OPTIONS}
            />
          </div>
          <Select
            ariaLabel="Bucket interval"
            value={q.interval}
            onValueChange={(interval) => updateQuery({ interval }, { mode: "push" })}
            options={INTERVAL_OPTIONS}
          />
          <Button
            variant={q.compare ? "primary" : "secondary"}
            aria-pressed={q.compare}
            onClick={() => updateQuery({ compare: !q.compare }, { mode: "push" })}
          >
            Compare
          </Button>
          <div className="flex items-center gap-1.5">
            <BrandIcon name="rm-filter" className="text-[var(--fg-muted)]" />
            <Input
              aria-label="Filter expression"
              placeholder="country:US,path:/blog/*"
              value={filterInput}
              className="w-44"
              onChange={(e) => {
                const raw = e.target.value;
                setFilterInput(raw);
                updateQuery({ filters: parseFiltersParam(raw) }, { mode: "replace" });
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter") {
                  updateQuery({ filters: parseFiltersParam(filterInput) }, { mode: "push" });
                }
              }}
            />
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-2">
          <span
            className="rm-tabular border border-[var(--border-hairline)] bg-[var(--bg-elevated)] px-2 py-1 text-[var(--text-xs)] text-[var(--fg-secondary)] rounded-[var(--radius-xs)]"
            title="Node topology & query latency"
          >
            {telemetryLabel}
          </span>
          <Select<ThemePreference>
            ariaLabel="Theme"
            value={prefs.theme}
            onValueChange={(theme) => onChangePrefs({ ...prefs, theme })}
            options={[
              { value: "system", label: "System" },
              { value: "light", label: "Light" },
              { value: "dark", label: "Dark" },
            ]}
          />
          <Select<DensityPreference>
            ariaLabel="Density"
            value={prefs.density}
            onValueChange={(density) => onChangePrefs({ ...prefs, density })}
            options={[
              { value: "compact", label: "Compact" },
              { value: "standard", label: "Standard" },
              { value: "comfortable", label: "Comfortable" },
            ]}
          />
          <Button variant="ghost" onClick={onLogout}>
            Sign out ({auth.subject})
          </Button>
        </div>
      </header>

      <div className="flex flex-1 min-h-0">
        {!prefs.sidebarCollapsed ? (
          <nav
            aria-label="Dashboard sections"
            className="w-48 shrink-0 border-r border-[var(--border-hairline)] bg-[var(--bg-surface)] p-2 flex flex-col gap-1"
          >
            {navItems.map((item) => {
              const active = activeRouteId === item.id;
              const icon = NAV_ICON_BY_ID[item.id] ?? "rm-range";
              return (
                <Button
                  key={item.id}
                  variant={active ? "primary" : "ghost"}
                  className="justify-start w-full"
                  onClick={() => onNavigatePlugin(item.path)}
                >
                  <BrandIcon name={icon} />
                  <span>{item.label}</span>
                </Button>
              );
            })}
          </nav>
        ) : null}
        <main className="flex-1 p-4 overflow-auto">{children}</main>
      </div>
    </div>
  );
}
