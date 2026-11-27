import { useEffect, useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  useApiClient,
  type SaltRotation,
  type SiteResponse,
} from "@rotameter/plugin-sdk";
import { Button, Dialog, Input, Select } from "@rotameter/ui";
import { buildSitePayload, emptySiteForm, siteToForm, type SiteFormState } from "./forms";

export interface SiteFormDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  site: SiteResponse | null;
}

const SALT_OPTIONS: ReadonlyArray<{ value: SaltRotation; label: string }> = [
  { value: "daily", label: "Daily (00:00 UTC)" },
  { value: "weekly", label: "Weekly (Monday UTC)" },
  { value: "monthly", label: "Monthly (1st UTC)" },
];

export function SiteFormDialog({ open, onOpenChange, site }: SiteFormDialogProps) {
  const api = useApiClient();
  const qc = useQueryClient();
  const mode = site ? "edit" : "create";

  const [form, setForm] = useState<SiteFormState>(() =>
    site ? siteToForm(site) : emptySiteForm(),
  );
  const [formError, setFormError] = useState<string | null>(null);

  useEffect(() => {
    if (open) {
      setForm(site ? siteToForm(site) : emptySiteForm());
      setFormError(null);
    }
  }, [open, site]);

  const mutation = useMutation({
    mutationFn: async () => {
      const built = buildSitePayload(form, mode);
      if (!built.ok) {
        throw new Error(built.error);
      }
      if (site) {
        return api.updateSite(site.site_id, built.payload);
      }
      return api.createSite(built.payload);
    },
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: ["admin-sites"] });
      void qc.invalidateQueries({ queryKey: ["sites"] });
      onOpenChange(false);
    },
    onError: (err) => {
      setFormError(err instanceof Error ? err.message : "Failed to save site");
    },
  });

  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    setFormError(null);
    mutation.mutate();
  };

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title={site ? `Edit Site · ${site.slug}` : "Register Site"}
      description={site ? `Site UID: ${site.site_id}` : "Create a new tracked site and configure privacy policy."}
    >
      <form onSubmit={handleSubmit} className="flex flex-col gap-3">
        <label className="flex flex-col gap-1 text-[var(--text-xs)] text-[var(--fg-secondary)]">
          <span>Slug</span>
          {site ? (
            <div className="rm-tabular border border-[var(--border-hairline)] bg-[var(--bg-sunken)] px-2.5 py-1.5 text-[var(--text-sm)] text-[var(--fg-primary)] rounded-[var(--radius-xs)]">
              {site.slug}
            </div>
          ) : (
            <Input
              aria-label="Site slug"
              placeholder="blog.example.com"
              value={form.slug}
              onChange={(e) => setForm((f) => ({ ...f, slug: e.target.value }))}
              required
            />
          )}
        </label>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
          <label className="flex flex-col gap-1 text-[var(--text-xs)] text-[var(--fg-secondary)]">
            <span>Display Name</span>
            <Input
              aria-label="Display name"
              placeholder="My Blog"
              value={form.display_name}
              onChange={(e) => setForm((f) => ({ ...f, display_name: e.target.value }))}
            />
          </label>
          <label className="flex flex-col gap-1 text-[var(--text-xs)] text-[var(--fg-secondary)]">
            <span>Timezone (IANA)</span>
            <Input
              aria-label="Timezone"
              placeholder="UTC"
              value={form.timezone}
              onChange={(e) => setForm((f) => ({ ...f, timezone: e.target.value }))}
            />
          </label>
        </div>

        <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
          <div className="flex flex-col gap-1 text-[var(--text-xs)] text-[var(--fg-secondary)]">
            <span>Salt Rotation</span>
            <Select<SaltRotation>
              ariaLabel="Salt rotation"
              value={form.salt_rotation}
              onValueChange={(salt_rotation) => setForm((f) => ({ ...f, salt_rotation }))}
              options={SALT_OPTIONS}
            />
          </div>
          <label className="flex flex-col gap-1 text-[var(--text-xs)] text-[var(--fg-secondary)]">
            <span>Raw Event Retention (days)</span>
            <Input
              aria-label="Raw event retention days"
              type="number"
              min={1}
              placeholder="Keep forever (blank)"
              value={form.raw_retention_days}
              onChange={(e) => setForm((f) => ({ ...f, raw_retention_days: e.target.value }))}
            />
          </label>
        </div>

        <label className="inline-flex items-center gap-2 min-h-[24px] py-1 text-[var(--text-sm)] text-[var(--fg-primary)] cursor-pointer">
          <input
            type="checkbox"
            checked={form.respect_gpc}
            onChange={(e) => setForm((f) => ({ ...f, respect_gpc: e.target.checked }))}
            className="h-4 w-4 accent-[var(--accent-primary)] cursor-pointer"
          />
          <span>Respect Global Privacy Control (`Sec-GPC: 1`)</span>
        </label>

        <label className="flex flex-col gap-1 text-[var(--text-xs)] text-[var(--fg-secondary)]">
          <span>Allowed Origins (comma or newline separated)</span>
          <textarea
            aria-label="Allowed origins"
            rows={2}
            placeholder="https://example.com"
            value={form.allowed_origins}
            onChange={(e) => setForm((f) => ({ ...f, allowed_origins: e.target.value }))}
            className="w-full border border-[var(--border-strong)] bg-[var(--bg-surface)] text-[var(--fg-primary)] px-2.5 py-1.5 text-[var(--text-sm)] rounded-[var(--radius-xs)] placeholder:text-[var(--fg-muted)]"
          />
        </label>

        <label className="flex flex-col gap-1 text-[var(--text-xs)] text-[var(--fg-secondary)]">
          <span>Allowed Query Keys (strip all others)</span>
          <textarea
            aria-label="Allowed query keys"
            rows={2}
            placeholder="utm_source, utm_medium, utm_campaign"
            value={form.allowed_query_keys}
            onChange={(e) => setForm((f) => ({ ...f, allowed_query_keys: e.target.value }))}
            className="w-full border border-[var(--border-strong)] bg-[var(--bg-surface)] text-[var(--fg-primary)] px-2.5 py-1.5 text-[var(--text-sm)] rounded-[var(--radius-xs)] placeholder:text-[var(--fg-muted)]"
          />
        </label>

        {formError ? (
          <p role="alert" className="text-[var(--text-sm)] text-[var(--signal-crit)]">
            {formError}
          </p>
        ) : null}

        <div className="flex justify-end gap-2 pt-1">
          <Button variant="ghost" type="button" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button variant="primary" type="submit" disabled={mutation.isPending}>
            {mutation.isPending ? "Saving…" : site ? "Save Changes" : "Create Site"}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
