import { useCallback, useEffect, useMemo, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  useNavigate,
  useRouterState,
} from "@tanstack/react-router";
import {
  DashboardProvider,
  loadEnabledPlugins,
  RotameterApiClient,
  type AuthMeResponse,
  type DashboardQueryState,
  type PluginManifest,
  type QueryMeta,
  type UpdateQueryOptions,
} from "@rotameter/plugin-sdk";
import { AppShell } from "./components/AppShell";
import { AuthGate } from "./components/AuthGate";
import { IngestRoleView } from "./components/IngestRoleView";
import { CORE_PLUGIN_LOADERS } from "./plugins";
import {
  applyPreferencesToDom,
  loadPreferences,
  savePreferences,
  type UiPreferences,
} from "./preferences";
import {
  searchToQueryState,
  serializeFiltersParam,
  validateDashboardSearch,
  type DashboardSearchParams,
} from "./searchParams";

const api = new RotameterApiClient();

function resolveActiveRouteId(pathname: string): string {
  if (pathname === "/admin") return "admin";
  if (pathname === "/cluster") return "cluster";
  if (pathname.endsWith("/breakdowns")) return "breakdowns";
  if (pathname.endsWith("/realtime")) return "realtime";
  return "overview";
}

function extractSiteParam(pathname: string): string | null {
  const match = /^\/sites\/([^/]+)/.exec(pathname);
  return match?.[1] ? decodeURIComponent(match[1]) : null;
}

function RootShell() {
  const qc = useQueryClient();
  const navigate = useNavigate();
  const location = useRouterState({ select: (s) => s.location });
  const [prefs, setPrefs] = useState<UiPreferences>(() => loadPreferences());
  const [telemetryMeta, setTelemetryMeta] = useState<QueryMeta | null>(null);
  const [plugins, setPlugins] = useState<PluginManifest[]>([]);

  useEffect(() => {
    applyPreferencesToDom(prefs);
    savePreferences(prefs);
  }, [prefs]);

  const capsQuery = useQuery({
    queryKey: ["capabilities"],
    queryFn: ({ signal }) => api.capabilities(signal),
    staleTime: 60_000,
  });

  const authQuery = useQuery({
    queryKey: ["auth-me"],
    queryFn: async ({ signal }) => {
      try {
        return await api.authMe(signal);
      } catch {
        return null;
      }
    },
    retry: false,
    enabled: Boolean(capsQuery.data),
  });

  const caps = capsQuery.data;
  const auth = authQuery.data ?? null;

  useEffect(() => {
    if (!caps || !auth) return;
    let cancelled = false;
    void loadEnabledPlugins(CORE_PLUGIN_LOADERS, caps, auth.scope).then((loaded) => {
      if (!cancelled) setPlugins(loaded);
    });
    return () => {
      cancelled = true;
    };
  }, [caps, auth]);

  const sitesQuery = useQuery({
    queryKey: ["sites"],
    queryFn: ({ signal }) => api.listSites(signal),
    enabled: Boolean(caps && caps.node_role !== "ingest" && auth?.scope === "admin"),
    staleTime: 60_000,
  });

  const sites = sitesQuery.data ?? [];
  const defaultSiteId = auth?.site_id ?? sites[0]?.slug ?? "_self";
  const siteId = extractSiteParam(location.pathname) ?? defaultSiteId;
  const search = useMemo(
    () => validateDashboardSearch((location.search as Record<string, unknown>) ?? {}),
    [location.search],
  );
  const queryState = useMemo(() => searchToQueryState(siteId, search), [siteId, search]);

  const activeRouteId = resolveActiveRouteId(location.pathname);

  const navigatePlugin = useCallback(
    (path: string, targetSite = siteId, nextSearch = search, replace = false) => {
      const dest = path.startsWith("/") ? path : `/sites/${encodeURIComponent(targetSite)}/${path}`;
      void navigate({ to: dest as never, search: nextSearch as never, replace });
    },
    [navigate, siteId, search],
  );

  const updateQuery = useCallback(
    (patch: Partial<DashboardQueryState>, opts?: UpdateQueryOptions) => {
      const nextSite = patch.siteId ?? siteId;
      const nextSearch: DashboardSearchParams = {
        range: patch.range ?? search.range,
        interval: patch.interval ?? search.interval,
        compare: patch.compare ?? search.compare,
        f: patch.filters ? serializeFiltersParam(patch.filters) : search.f,
      };
      const path =
        activeRouteId === "admin" || activeRouteId === "cluster"
          ? `/${activeRouteId}`
          : activeRouteId;
      navigatePlugin(path, nextSite, nextSearch, opts?.mode === "replace");
    },
    [siteId, search, activeRouteId, navigatePlugin],
  );

  useEffect(() => {
    if (caps && auth && caps.node_role !== "ingest" && location.pathname === "/") {
      navigatePlugin("overview", defaultSiteId, search, true);
    }
  }, [caps, auth, location.pathname, defaultSiteId, search, navigatePlugin]);

  if (capsQuery.isPending || (caps && authQuery.isPending)) {
    return (
      <main className="min-h-screen flex items-center justify-center bg-[var(--bg-canvas)] text-[var(--fg-muted)]">
        Initializing Rotameter capabilities…
      </main>
    );
  }

  if (!caps) {
    return (
      <main className="min-h-screen flex items-center justify-center bg-[var(--bg-canvas)] text-[var(--signal-crit)]">
        Failed to reach `/v1/capabilities`.
      </main>
    );
  }

  if (caps.node_role === "ingest") {
    return <IngestRoleView api={api} capabilities={caps} />;
  }

  if (!auth) {
    return (
      <AuthGate
        api={api}
        capabilities={caps}
        onAuthenticated={(nextAuth: AuthMeResponse) => {
          qc.setQueryData(["auth-me"], nextAuth);
        }}
      />
    );
  }

  const ActiveComponent = plugins
    .flatMap((p) => p.routes ?? [])
    .find((r) => r.id === activeRouteId)?.component;

  return (
    <DashboardProvider
      value={{
        api,
        capabilities: caps,
        auth,
        query: queryState,
        updateQuery,
        telemetryMeta,
        setTelemetryMeta,
      }}
    >
      <AppShell
        plugins={plugins}
        sites={sites}
        activeRouteId={activeRouteId}
        onNavigatePlugin={(p) => navigatePlugin(p)}
        onSelectSite={(nextSite) => navigatePlugin("overview", nextSite)}
        prefs={prefs}
        onChangePrefs={setPrefs}
        onLogout={() => {
          void api.logout().finally(() => {
            qc.setQueryData(["auth-me"], null);
            void qc.invalidateQueries({ queryKey: ["sites"] });
          });
        }}
      >
        {ActiveComponent ? <ActiveComponent /> : <Outlet />}
      </AppShell>
    </DashboardProvider>
  );
}

const rootRoute = createRootRoute({ component: RootShell });
const emptyOutlet = () => null;

const routeTree = rootRoute.addChildren([
  createRoute({ getParentRoute: () => rootRoute, path: "/", component: emptyOutlet }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: "/sites/$siteId/overview",
    validateSearch: validateDashboardSearch,
    component: emptyOutlet,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: "/sites/$siteId/breakdowns",
    validateSearch: validateDashboardSearch,
    component: emptyOutlet,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: "/sites/$siteId/realtime",
    validateSearch: validateDashboardSearch,
    component: emptyOutlet,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: "/admin",
    validateSearch: validateDashboardSearch,
    component: emptyOutlet,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: "/cluster",
    validateSearch: validateDashboardSearch,
    component: emptyOutlet,
  }),
]);

export const router = createRouter({ routeTree });
