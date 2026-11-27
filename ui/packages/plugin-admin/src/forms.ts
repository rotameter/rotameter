import type {
  CreateKeyRequest,
  KeyScope,
  SaltRotation,
  SiteResponse,
  SiteUpsertRequest,
} from "@rotameter/plugin-sdk";

export interface SiteFormState {
  slug: string;
  display_name: string;
  timezone: string;
  salt_rotation: SaltRotation;
  respect_gpc: boolean;
  allowed_origins: string;
  allowed_query_keys: string;
  raw_retention_days: string;
}

export function parseListInput(raw: string): string[] {
  const seen = new Set<string>();
  const out: string[] = [];
  for (const part of raw.split(/[\n,]+/)) {
    const trimmed = part.trim();
    if (trimmed.length > 0 && !seen.has(trimmed)) {
      seen.add(trimmed);
      out.push(trimmed);
    }
  }
  return out;
}

export function emptySiteForm(): SiteFormState {
  return {
    slug: "",
    display_name: "",
    timezone: "UTC",
    salt_rotation: "daily",
    respect_gpc: false,
    allowed_origins: "",
    allowed_query_keys: "",
    raw_retention_days: "",
  };
}

export function siteToForm(site: SiteResponse): SiteFormState {
  return {
    slug: site.slug,
    display_name: site.display_name,
    timezone: site.timezone,
    salt_rotation: site.salt_rotation,
    respect_gpc: site.respect_gpc,
    allowed_origins: site.allowed_origins.join("\n"),
    allowed_query_keys: site.allowed_query_keys.join("\n"),
    raw_retention_days: site.raw_retention_days === null ? "" : String(site.raw_retention_days),
  };
}

export function buildSitePayload(
  form: SiteFormState,
  mode: "create" | "edit",
): { ok: true; payload: SiteUpsertRequest } | { ok: false; error: string } {
  const slug = form.slug.trim();
  if (mode === "create" && slug.length === 0) {
    return { ok: false, error: "Site slug is required." };
  }

  const retentionRaw = form.raw_retention_days.trim();
  let retentionDays: number | undefined;
  if (retentionRaw.length > 0) {
    if (!/^\d+$/.test(retentionRaw)) {
      return { ok: false, error: "Raw retention days must be a positive whole number." };
    }
    const parsed = Number.parseInt(retentionRaw, 10);
    if (!Number.isFinite(parsed) || parsed <= 0) {
      return { ok: false, error: "Raw retention days must be greater than zero." };
    }
    retentionDays = parsed;
  }

  const displayName = form.display_name.trim();
  const timezone = form.timezone.trim() || "UTC";

  const payload: SiteUpsertRequest = {
    ...(mode === "create" ? { slug } : {}),
    ...(displayName.length > 0 ? { display_name: displayName } : {}),
    timezone,
    salt_rotation: form.salt_rotation,
    respect_gpc: form.respect_gpc,
    allowed_origins: parseListInput(form.allowed_origins),
    allowed_query_keys: parseListInput(form.allowed_query_keys),
    ...(retentionDays !== undefined ? { raw_retention_days: retentionDays } : {}),
  };
  return { ok: true, payload };
}

export function validateKeyForm(input: {
  label: string;
  scope: KeyScope;
  site: string;
}): { ok: true; payload: CreateKeyRequest } | { ok: false; error: string } {
  const label = input.label.trim();
  if (label.length === 0 || label.length > 128) {
    return { ok: false, error: "Key label must be 1 to 128 characters." };
  }
  const site = input.site.trim();
  if (input.scope === "admin") {
    return { ok: true, payload: { scope: "admin", label } };
  }
  if (site.length === 0) {
    return { ok: false, error: "Select a site for read/ingest API keys." };
  }
  return { ok: true, payload: { scope: input.scope, site, label } };
}

export function buildCollectSnippet(origin: string, siteId: string): string {
  const cleanOrigin = origin.replace(/\/+$/, "");
  const scriptTag = `<script defer src="${cleanOrigin}/v1/collect.js" data-site="${siteId}" data-endpoint="${cleanOrigin}"></script>`;
  const noscriptTag = `<noscript><img src="${cleanOrigin}/v1/pixel.gif?site=${siteId}" alt="" width="1" height="1"></noscript>`;
  return `${scriptTag}\n${noscriptTag}`;
}

export function resolveOrigin(apiBase: string): string {
  const trimmed = apiBase.trim().replace(/\/+$/, "");
  if (trimmed.length > 0) return trimmed;
  if (typeof document !== "undefined" && document.baseURI) {
    return new URL(document.baseURI).origin;
  }
  return "";
}
