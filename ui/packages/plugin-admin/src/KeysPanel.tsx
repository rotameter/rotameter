import { useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useApiClient, type ApiKeyMetadata } from "@rotameter/plugin-sdk";
import { Button, Dialog } from "@rotameter/ui";
import { KeyCreateDialog } from "./KeyCreateDialog";

export function KeysPanel() {
  const api = useApiClient();
  const qc = useQueryClient();

  const [showRevoked, setShowRevoked] = useState(false);
  const [createOpen, setCreateOpen] = useState(false);
  const [revokingKey, setRevokingKey] = useState<ApiKeyMetadata | null>(null);
  const [revokeError, setRevokeError] = useState<string | null>(null);

  const keysQuery = useQuery({
    queryKey: ["admin-keys"],
    queryFn: ({ signal }) => api.listKeys(signal),
    staleTime: 30_000,
  });

  const sitesQuery = useQuery({
    queryKey: ["admin-sites"],
    queryFn: ({ signal }) => api.listSites(signal),
    staleTime: 30_000,
  });

  const revokeMutation = useMutation({
    mutationFn: (keyId: string) => api.revokeKey(keyId),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["admin-keys"] });
      setRevokingKey(null);
      setRevokeError(null);
    },
    onError: (err) => {
      setRevokeError(err instanceof Error ? err.message : "Failed to revoke key");
    },
  });

  const siteSlugById = useMemo(() => {
    const map = new Map<string, string>();
    for (const s of sitesQuery.data ?? []) {
      map.set(s.site_id, s.slug);
    }
    return map;
  }, [sitesQuery.data]);

  const visibleKeys = useMemo(() => {
    const all = keysQuery.data ?? [];
    return showRevoked ? all : all.filter((k) => k.revoked_at === null);
  }, [keysQuery.data, showRevoked]);

  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex items-center gap-2">
          <Button
            variant={showRevoked ? "secondary" : "ghost"}
            aria-pressed={showRevoked}
            onClick={() => setShowRevoked((v) => !v)}
          >
            {showRevoked ? "Showing Revoked" : "Show Revoked"}
          </Button>
          <span className="text-[var(--text-xs)] text-[var(--fg-muted)]">
            {visibleKeys.length} key(s)
          </span>
        </div>
        <Button variant="primary" onClick={() => setCreateOpen(true)}>
          New API Key
        </Button>
      </div>

      {keysQuery.isPending ? (
        <p className="text-[var(--fg-muted)]">Loading API keys…</p>
      ) : keysQuery.error ? (
        <p className="text-[var(--signal-crit)]">{keysQuery.error.message}</p>
      ) : visibleKeys.length === 0 ? (
        <p className="border border-[var(--border-hairline)] bg-[var(--bg-sunken)] px-3 py-4 text-center text-[var(--fg-muted)]">
          No active API keys matching filter.
        </p>
      ) : (
        <div className="overflow-x-auto border border-[var(--border-hairline)]">
          <table className="w-full border-collapse text-[var(--text-sm)]" aria-label="API keys">
            <thead>
              <tr className="bg-[var(--bg-elevated)] border-b border-[var(--border-hairline)] text-[var(--text-xs)] uppercase tracking-[0.05em] text-[var(--fg-secondary)]">
                <th className="px-2.5 py-1.5 text-left font-medium">Label</th>
                <th className="px-2.5 py-1.5 text-left font-medium">Scope</th>
                <th className="px-2.5 py-1.5 text-left font-medium">Site</th>
                <th className="px-2.5 py-1.5 text-left font-medium">Prefix</th>
                <th className="px-2.5 py-1.5 text-left font-medium">Last Used</th>
                <th className="px-2.5 py-1.5 text-left font-medium">Status</th>
                <th className="px-2.5 py-1.5 text-right font-medium">Action</th>
              </tr>
            </thead>
            <tbody>
              {visibleKeys.map((k) => {
                const revoked = k.revoked_at !== null;
                const siteLabel = k.site_id ? (siteSlugById.get(k.site_id) ?? k.site_id) : "org-wide";
                return (
                  <tr key={k.key_id} className="border-b border-[var(--border-hairline)] last:border-b-0">
                    <td className="px-2.5 py-2 font-medium text-[var(--fg-primary)]">{k.label}</td>
                    <td className="px-2.5 py-2 rm-tabular text-[var(--fg-secondary)]">{k.scope}</td>
                    <td className="px-2.5 py-2 rm-tabular text-[var(--fg-secondary)]">{siteLabel}</td>
                    <td className="px-2.5 py-2 rm-tabular text-[var(--fg-muted)]">{k.key_prefix}…</td>
                    <td className="px-2.5 py-2 rm-tabular text-[var(--fg-muted)]">
                      {k.last_used_at ?? "—"}
                    </td>
                    <td
                      className={`px-2.5 py-2 text-[var(--text-xs)] font-semibold ${
                        revoked ? "text-[var(--signal-crit)]" : "text-[var(--signal-ok)]"
                      }`}
                    >
                      {revoked ? "REVOKED" : "ACTIVE"}
                    </td>
                    <td className="px-2.5 py-2 text-right">
                      {revoked ? (
                        <span className="text-[var(--text-xs)] text-[var(--fg-muted)]">—</span>
                      ) : (
                        <Button
                          variant="danger"
                          onClick={() => {
                            setRevokeError(null);
                            setRevokingKey(k);
                          }}
                        >
                          Revoke
                        </Button>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      <KeyCreateDialog
        open={createOpen}
        onOpenChange={setCreateOpen}
        sites={sitesQuery.data ?? []}
      />

      <Dialog
        open={Boolean(revokingKey)}
        onOpenChange={(open) => {
          if (!open) setRevokingKey(null);
        }}
        title={`Revoke API Key · ${revokingKey?.label ?? ""}`}
        description={`Prefix ${revokingKey?.key_prefix ?? ""}… will stop authenticating immediately.`}
      >
        <div className="flex flex-col gap-3">
          {revokeError ? (
            <p role="alert" className="text-[var(--text-sm)] text-[var(--signal-crit)]">
              {revokeError}
            </p>
          ) : null}
          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setRevokingKey(null)}>
              Cancel
            </Button>
            <Button
              variant="danger"
              disabled={!revokingKey || revokeMutation.isPending}
              onClick={() => {
                if (revokingKey) revokeMutation.mutate(revokingKey.key_id);
              }}
            >
              {revokeMutation.isPending ? "Revoking…" : "Confirm Revoke"}
            </Button>
          </div>
        </div>
      </Dialog>
    </div>
  );
}
