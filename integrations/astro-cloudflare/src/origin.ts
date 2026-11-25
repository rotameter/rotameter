import type {
  AllowedOriginsOptions,
  CloudflareRequestCf,
  NormalizedBeaconPayload,
  PropValue,
  RotameterEnv,
} from "./types.ts";

export function baseUrl(raw: string): string {
  return raw.trim().replace(/\/+$/, "");
}

export function parseHostname(raw: string | null | undefined): string | null {
  const trimmed = raw?.trim();
  if (!trimmed) return null;
  try {
    const candidate = trimmed.includes("://") ? trimmed : `https://${trimmed}`;
    const host = new URL(candidate).hostname.toLowerCase();
    return host || null;
  } catch {
    return null;
  }
}

export function isLocalHost(host: string | null | undefined): boolean {
  if (!host) return true;
  const h = host.toLowerCase();
  return (
    h === "localhost" ||
    h === "127.0.0.1" ||
    h === "::1" ||
    h === "[::1]" ||
    h.endsWith(".localhost")
  );
}

export function isPreviewHost(host: string | null | undefined): boolean {
  if (!host) return false;
  const h = host.toLowerCase();
  return h.endsWith(".workers.dev") || h.endsWith(".pages.dev");
}

export function isPreviewOrLocalHost(host: string | null | undefined): boolean {
  return isLocalHost(host) || isPreviewHost(host);
}

export function parseAllowedOrigins(
  raw: string | undefined,
  opts: AllowedOriginsOptions = {},
): Set<string> {
  const out = new Set<string>();
  if (!raw) return out;
  for (const part of raw.split(",")) {
    const host = parseHostname(part);
    if (host && (opts.allowLocal || !isLocalHost(host))) {
      out.add(host);
    }
  }
  return out;
}

export function extractRequestHost(request: Request, payloadUrl?: string): string | null {
  return (
    parseHostname(request.headers.get("origin")) ??
    parseHostname(request.headers.get("referer")) ??
    parseHostname(payloadUrl) ??
    parseHostname(request.url)
  );
}

export function isAllowedAnalyticsOrigin(
  request: Request,
  allowed: ReadonlySet<string>,
  payloadUrl?: string,
  opts: AllowedOriginsOptions = {},
): boolean {
  if (allowed.size === 0) return false;
  const reqHost = extractRequestHost(request, payloadUrl);
  if (!reqHost || (!opts.allowLocal && isLocalHost(reqHost)) || !allowed.has(reqHost)) {
    return false;
  }
  if (payloadUrl !== undefined) {
    const payloadHost = parseHostname(payloadUrl);
    if (
      !payloadHost ||
      (!opts.allowLocal && isLocalHost(payloadHost)) ||
      !allowed.has(payloadHost)
    ) {
      return false;
    }
  }
  return true;
}

export function sanitizeProps(raw: unknown): Record<string, PropValue> | undefined {
  if (!raw || typeof raw !== "object" || Array.isArray(raw)) return undefined;
  const clean: Record<string, PropValue> = {};
  for (const [k, v] of Object.entries(raw as Record<string, unknown>)) {
    if (typeof v === "string" || typeof v === "number" || typeof v === "boolean") {
      clean[k] = v;
    }
  }
  return Object.keys(clean).length > 0 ? clean : undefined;
}

export function parseBeaconPayload(
  rawBody: string,
  siteId: string,
): NormalizedBeaconPayload | null {
  let parsed: unknown;
  try {
    parsed = JSON.parse(rawBody);
  } catch {
    return null;
  }
  if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return null;
  const obj = parsed as Record<string, unknown>;
  if (typeof obj.url !== "string" || obj.url.length === 0 || obj.url.length > 2048) return null;

  const out: NormalizedBeaconPayload = { site_id: siteId, url: obj.url };
  if (typeof obj.event === "string" && obj.event.trim().length > 0) out.event = obj.event.trim();
  if (typeof obj.referrer === "string" && obj.referrer.trim().length > 0) {
    out.referrer = obj.referrer.trim();
  }
  if (
    typeof obj.viewport_width === "number" &&
    Number.isInteger(obj.viewport_width) &&
    obj.viewport_width >= 0 &&
    obj.viewport_width <= 65_535
  ) {
    out.viewport_width = obj.viewport_width;
  }
  const cleanProps = sanitizeProps(obj.props);
  if (cleanProps) out.props = cleanProps;
  return out;
}

export function buildAccessHeaders(env: RotameterEnv): Record<string, string> {
  const headers: Record<string, string> = {};
  const id = env.ROTAMETER_ACCESS_CLIENT_ID?.trim() || env.CF_ACCESS_CLIENT_ID?.trim();
  const secret = env.ROTAMETER_ACCESS_CLIENT_SECRET?.trim() || env.CF_ACCESS_CLIENT_SECRET?.trim();
  if (id && secret) {
    headers["CF-Access-Client-Id"] = id;
    headers["CF-Access-Client-Secret"] = secret;
  }
  return headers;
}

export function buildForwardHeaders(request: Request, env: RotameterEnv): Record<string, string> {
  const headers: Record<string, string> = {
    Authorization: `Bearer ${(env.ROTAMETER_INGEST_KEY ?? "").trim()}`,
    "Content-Type": "application/json",
    ...buildAccessHeaders(env),
  };
  const cf = (request as Request & { cf?: CloudflareRequestCf }).cf;
  const ua = request.headers.get("user-agent")?.trim();
  const gpc = request.headers.get("sec-gpc")?.trim();
  const ip = request.headers.get("cf-connecting-ip")?.trim();
  const country = (request.headers.get("cf-ipcountry") ?? cf?.country)?.trim();
  const region = (
    request.headers.get("cf-region-code") ??
    cf?.regionCode ??
    cf?.region
  )?.trim();

  if (ua) headers["User-Agent"] = ua;
  if (gpc) headers["Sec-GPC"] = gpc;
  if (ip) headers["X-Rotameter-Client-IP"] = ip;
  if (country) headers["X-Rotameter-Country"] = country;
  if (region) headers["X-Rotameter-Region"] = region;
  return headers;
}
