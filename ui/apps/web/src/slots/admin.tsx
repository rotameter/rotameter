import { useQuery } from "@tanstack/react-query";
import { useApiClient, type PluginManifest } from "@rotameter/plugin-sdk";
import { InstrumentFrame } from "@rotameter/ui";

function AdminSlot() {
  const api = useApiClient();
  const { data, isPending, error } = useQuery({
    queryKey: ["admin-sites"],
    queryFn: ({ signal }) => api.listSites(signal),
    staleTime: 30_000,
  });

  return (
    <InstrumentFrame title="Admin" subtitle="Sites & API Keys (RM-40 slot)">
      {isPending ? (
        <p className="text-[var(--fg-muted)]">Loading sites…</p>
      ) : error ? (
        <p className="text-[var(--signal-crit)]">{error.message}</p>
      ) : (
        <ul className="divide-y divide-[var(--border-hairline)]">
          {data.map((site) => (
            <li key={site.site_id} className="flex items-center justify-between py-1.5 text-[var(--text-sm)]">
              <span className="font-medium text-[var(--fg-primary)]">{site.slug}</span>
              <span className="rm-tabular text-[var(--text-xs)] text-[var(--fg-muted)]">
                {site.site_id} · {site.timezone}
              </span>
            </li>
          ))}
        </ul>
      )}
    </InstrumentFrame>
  );
}

const manifest: PluginManifest = {
  id: "admin",
  name: "Admin",
  version: "0.3.0",
  required_scopes: ["admin:sites", "admin:keys"],
  nav_items: [{ id: "admin", label: "Admin", path: "/admin", order: 80 }],
  routes: [{ id: "admin", path: "/admin", component: AdminSlot }],
};

export default manifest;
