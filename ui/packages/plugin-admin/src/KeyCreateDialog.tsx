import { useEffect, useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  useApiClient,
  type CreatedApiKey,
  type KeyScope,
  type SiteResponse,
} from "@rotameter/plugin-sdk";
import { Button, Dialog, Input, Select } from "@rotameter/ui";
import { validateKeyForm } from "./forms";

export interface KeyCreateDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  sites: SiteResponse[];
}

const SCOPE_OPTIONS: ReadonlyArray<{ value: KeyScope; label: string }> = [
  { value: "read", label: "Read (`rm_rd_`)" },
  { value: "ingest", label: "Ingest (`rm_ing_`)" },
  { value: "admin", label: "Admin (`rm_adm_`)" },
];

export function KeyCreateDialog({ open, onOpenChange, sites }: KeyCreateDialogProps) {
  const api = useApiClient();
  const qc = useQueryClient();

  const [label, setLabel] = useState("");
  const [scope, setScope] = useState<KeyScope>("read");
  const [site, setSite] = useState("");
  const [formError, setFormError] = useState<string | null>(null);
  const [created, setCreated] = useState<CreatedApiKey | null>(null);
  const [copied, setCopied] = useState(false);

  useEffect(() => {
    if (open) {
      setLabel("");
      setScope("read");
      setSite(sites[0]?.site_id ?? "");
      setFormError(null);
      setCreated(null);
      setCopied(false);
    }
  }, [open, sites]);

  const mutation = useMutation({
    mutationFn: async () => {
      const check = validateKeyForm({ label, scope, site });
      if (!check.ok) throw new Error(check.error);
      return api.createKey(check.payload);
    },
    onSuccess: (res) => {
      setCreated(res);
      void qc.invalidateQueries({ queryKey: ["admin-keys"] });
    },
    onError: (err) => {
      setFormError(err instanceof Error ? err.message : "Failed to mint API key");
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
      title={created ? "API Key Minted" : "Create Scoped API Key"}
      description={
        created
          ? `Scope: ${created.scope} · Prefix: ${created.key_prefix}`
          : "Mint an ingest, read, or org-wide admin Bearer token."
      }
    >
      {created ? (
        <div className="flex flex-col gap-3">
          <div className="border border-[var(--signal-warn)] bg-[var(--bg-elevated)] px-2.5 py-2 text-[var(--text-xs)] text-[var(--signal-warn)]">
            This token is shown once. Store it now.
          </div>
          <Input
            readOnly
            aria-label="Minted plaintext API token"
            value={created.token}
            className="rm-tabular bg-[var(--bg-sunken)]"
          />
          <div className="flex justify-end gap-2">
            <Button
              variant="secondary"
              onClick={() => {
                if (typeof navigator !== "undefined" && navigator.clipboard) {
                  void navigator.clipboard.writeText(created.token).then(() => setCopied(true));
                }
              }}
            >
              {copied ? "Copied" : "Copy Token"}
            </Button>
            <Button variant="primary" onClick={() => onOpenChange(false)}>
              Done
            </Button>
          </div>
        </div>
      ) : (
        <form onSubmit={handleSubmit} className="flex flex-col gap-3">
          <label className="flex flex-col gap-1 text-[var(--text-xs)] text-[var(--fg-secondary)]">
            <span>Label</span>
            <Input
              aria-label="Key label"
              placeholder="ci-deploy or web-dashboard"
              value={label}
              onChange={(e) => setLabel(e.target.value)}
              required
            />
          </label>

          <div className="flex flex-col gap-1 text-[var(--text-xs)] text-[var(--fg-secondary)]">
            <span>Scope</span>
            <Select<KeyScope>
              ariaLabel="Key scope"
              value={scope}
              onValueChange={setScope}
              options={SCOPE_OPTIONS}
            />
          </div>

          {scope !== "admin" ? (
            <div className="flex flex-col gap-1 text-[var(--text-xs)] text-[var(--fg-secondary)]">
              <span>Bound Site</span>
              {sites.length > 0 ? (
                <Select
                  ariaLabel="Bound site"
                  value={site}
                  onValueChange={setSite}
                  options={sites.map((s) => ({ value: s.site_id, label: `${s.slug} (${s.site_id})` }))}
                />
              ) : (
                <p className="text-[var(--text-xs)] text-[var(--signal-warn)]">
                  Create a site first before minting site-scoped keys.
                </p>
              )}
            </div>
          ) : null}

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
              {mutation.isPending ? "Minting…" : "Mint Key"}
            </Button>
          </div>
        </form>
      )}
    </Dialog>
  );
}
