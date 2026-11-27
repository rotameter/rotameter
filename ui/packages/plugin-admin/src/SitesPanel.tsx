import { useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useApiClient, type SiteResponse } from "@rotameter/plugin-sdk";
import { BrandIcon, Button, Dialog, Input } from "@rotameter/ui";
import { SiteFormDialog } from "./SiteFormDialog";
import { buildCollectSnippet, resolveOrigin } from "./forms";

export function SitesPanel() {
  const api = useApiClient();
  const qc = useQueryClient();

  const [formOpen, setFormOpen] = useState(false);
  const [editingSite, setEditingSite] = useState<SiteResponse | null>(null);
  const [snippetSite, setSnippetSite] = useState<SiteResponse | null>(null);
  const [copiedSnippet, setCopiedSnippet] = useState(false);
  const [deletingSite, setDeletingSite] = useState<SiteResponse | null>(null);
  const [confirmSlug, setConfirmSlug] = useState("");
  const [deleteError, setDeleteError] = useState<string | null>(null);

  const { data: sites, isPending, error } = useQuery({
    queryKey: ["admin-sites"],
    queryFn: ({ signal }) => api.listSites(signal),
    staleTime: 30_000,
  });

  const deleteMutation = useMutation({
    mutationFn: (siteId: string) => api.deleteSite(siteId),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["admin-sites"] });
      void qc.invalidateQueries({ queryKey: ["sites"] });
      setDeletingSite(null);
      setConfirmSlug("");
      setDeleteError(null);
    },
    onError: (err) => {
      setDeleteError(err instanceof Error ? err.message : "Failed to delete site");
    },
  });

  const snippetText = snippetSite
    ? buildCollectSnippet(resolveOrigin(api.query_base_url), snippetSite.site_id)
    : "";

  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <span className="inline-flex items-center gap-1.5 text-[var(--text-xs)] text-[var(--fg-muted)]">
          <BrandIcon name="rm-sites" className="text-[var(--fg-muted)]" />
          <span>{sites ? `${sites.length} registered site(s)` : "Loading registry…"}</span>
        </span>
        <Button
          variant="primary"
          onClick={() => {
            setEditingSite(null);
            setFormOpen(true);
          }}
        >
          New Site
        </Button>
      </div>

      {isPending ? (
        <p className="text-[var(--fg-muted)]">Loading sites…</p>
      ) : error ? (
        <p className="text-[var(--signal-crit)]">{error.message}</p>
      ) : sites.length === 0 ? (
        <p className="border border-[var(--border-hairline)] bg-[var(--bg-sunken)] px-3 py-4 text-center text-[var(--fg-muted)]">
          No sites registered yet.
        </p>
      ) : (
        <div className="overflow-x-auto border border-[var(--border-hairline)]">
          <table className="w-full border-collapse text-[var(--text-sm)]" aria-label="Tracked sites">
            <thead>
              <tr className="bg-[var(--bg-elevated)] border-b border-[var(--border-hairline)] text-[var(--text-xs)] uppercase tracking-[0.05em] text-[var(--fg-secondary)]">
                <th className="px-2.5 py-1.5 text-left font-medium">Slug / Name</th>
                <th className="px-2.5 py-1.5 text-left font-medium">Timezone</th>
                <th className="px-2.5 py-1.5 text-left font-medium">Salt</th>
                <th className="px-2.5 py-1.5 text-left font-medium">GPC</th>
                <th className="px-2.5 py-1.5 text-right font-medium">Retention</th>
                <th className="px-2.5 py-1.5 text-right font-medium">Actions</th>
              </tr>
            </thead>
            <tbody>
              {sites.map((site) => (
                <tr key={site.site_id} className="border-b border-[var(--border-hairline)] last:border-b-0">
                  <td className="px-2.5 py-2">
                    <div className="flex items-center gap-2">
                      <span className="font-medium text-[var(--fg-primary)]">{site.slug}</span>
                      {site.is_self_site ? (
                        <span className="border border-[var(--border-strong)] bg-[var(--bg-elevated)] px-1.5 py-0.5 text-[var(--text-xs)] text-[var(--fg-secondary)] rounded-[var(--radius-xs)]">
                          SELF
                        </span>
                      ) : null}
                    </div>
                    <div className="rm-tabular text-[var(--text-xs)] text-[var(--fg-muted)]">
                      {site.display_name} · {site.site_id}
                    </div>
                  </td>
                  <td className="px-2.5 py-2 rm-tabular text-[var(--fg-secondary)]">{site.timezone}</td>
                  <td className="px-2.5 py-2 text-[var(--fg-secondary)]">{site.salt_rotation}</td>
                  <td className="px-2.5 py-2 text-[var(--fg-secondary)]">
                    {site.respect_gpc ? "Respect" : "Off"}
                  </td>
                  <td className="px-2.5 py-2 text-right rm-tabular text-[var(--fg-secondary)]">
                    {site.raw_retention_days === null ? "∞" : `${site.raw_retention_days}d`}
                  </td>
                  <td className="px-2.5 py-2 text-right">
                    <div className="inline-flex items-center justify-end gap-1.5">
                      <Button
                        variant="ghost"
                        onClick={() => {
                          setCopiedSnippet(false);
                          setSnippetSite(site);
                        }}
                      >
                        Snippet
                      </Button>
                      <Button
                        variant="secondary"
                        onClick={() => {
                          setEditingSite(site);
                          setFormOpen(true);
                        }}
                      >
                        Edit
                      </Button>
                      <Button
                        variant="danger"
                        onClick={() => {
                          setConfirmSlug("");
                          setDeleteError(null);
                          setDeletingSite(site);
                        }}
                      >
                        Delete
                      </Button>
                    </div>
                  </td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}

      <SiteFormDialog open={formOpen} onOpenChange={setFormOpen} site={editingSite} />

      <Dialog
        open={Boolean(snippetSite)}
        onOpenChange={(open) => {
          if (!open) setSnippetSite(null);
        }}
        title={`Collector Snippet · ${snippetSite?.slug ?? ""}`}
        description="Cookieless first-party beacon (≤ 1.5 KB gzip) + zero-JS pixel fallback."
      >
        <div className="flex flex-col gap-3">
          <textarea
            readOnly
            aria-label="Collector HTML snippet"
            rows={4}
            value={snippetText}
            className="w-full rm-tabular border border-[var(--border-strong)] bg-[var(--bg-sunken)] text-[var(--fg-primary)] p-2.5 text-[var(--text-xs)] rounded-[var(--radius-xs)]"
          />
          <div className="flex justify-end gap-2">
            <Button
              variant="primary"
              onClick={() => {
                if (typeof navigator !== "undefined" && navigator.clipboard) {
                  void navigator.clipboard.writeText(snippetText).then(() => setCopiedSnippet(true));
                }
              }}
            >
              {copiedSnippet ? "Copied" : "Copy Snippet"}
            </Button>
          </div>
        </div>
      </Dialog>

      <Dialog
        open={Boolean(deletingSite)}
        onOpenChange={(open) => {
          if (!open) setDeletingSite(null);
        }}
        title={`Delete Site · ${deletingSite?.slug ?? ""}`}
        description="This permanently removes the site record and cascades to bound keys/events. Type the slug to confirm."
      >
        <div className="flex flex-col gap-3">
          <Input
            aria-label="Confirm site slug"
            placeholder={deletingSite?.slug ?? ""}
            value={confirmSlug}
            onChange={(e) => setConfirmSlug(e.target.value)}
          />
          {deleteError ? (
            <p role="alert" className="text-[var(--text-sm)] text-[var(--signal-crit)]">
              {deleteError}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setDeletingSite(null)}>
              Cancel
            </Button>
            <Button
              variant="danger"
              disabled={!deletingSite || confirmSlug.trim() !== deletingSite.slug || deleteMutation.isPending}
              onClick={() => {
                if (deletingSite) deleteMutation.mutate(deletingSite.site_id);
              }}
            >
              {deleteMutation.isPending ? "Deleting…" : "Confirm Delete"}
            </Button>
          </div>
        </div>
      </Dialog>
    </div>
  );
}
