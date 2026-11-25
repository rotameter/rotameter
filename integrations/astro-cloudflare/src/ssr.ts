import {
  baseUrl,
  buildForwardHeaders,
  isLocalHost,
  isPreviewHost,
  parseAllowedOrigins,
  parseHostname,
  sanitizeProps,
} from "./origin.ts";
import type {
  ExecutionContextLike,
  NormalizedBeaconPayload,
  RotameterEnv,
  ServerHitOptions,
} from "./types.ts";

export async function trackServerHit(
  request: Request,
  env: RotameterEnv,
  ctx?: ExecutionContextLike,
  opts: ServerHitOptions = {},
): Promise<boolean> {
  const upstream = env.ROTAMETER_URL?.trim();
  const ingestKey = env.ROTAMETER_INGEST_KEY?.trim();
  const siteId = env.ROTAMETER_SITE?.trim();
  if (!upstream || !ingestKey || !siteId) return false;

  const targetUrl = opts.url ?? request.url;
  const hitHost = parseHostname(targetUrl) ?? parseHostname(request.headers.get("host"));
  if (!hitHost || (!opts.allowLocal && isLocalHost(hitHost))) return false;

  const hasAllowlist = Boolean(env.ROTAMETER_ALLOWED_ORIGINS?.trim());
  const allowed = parseAllowedOrigins(env.ROTAMETER_ALLOWED_ORIGINS, opts);
  if (hasAllowlist) {
    if (!allowed.has(hitHost)) return false;
  } else if (isPreviewHost(hitHost)) {
    return false;
  }

  const referrer = opts.referrer ?? request.headers.get("referer")?.trim();
  const props = sanitizeProps(opts.props);
  const payload: NormalizedBeaconPayload = {
    site_id: siteId,
    event: opts.event?.trim() || "pageview",
    url: targetUrl,
    ...(referrer ? { referrer } : {}),
    ...(props ? { props } : {}),
  };

  const fetcher = opts.fetcher ?? fetch;
  const timeoutMs = opts.timeoutMs ?? 1500;
  const work = (async (): Promise<boolean> => {
    try {
      const res = await fetcher(`${baseUrl(upstream)}/v1/events`, {
        method: "POST",
        headers: buildForwardHeaders(request, env),
        body: JSON.stringify(payload),
        signal: AbortSignal.timeout(timeoutMs),
      });
      return res.ok;
    } catch {
      return false;
    }
  })();

  if (ctx?.waitUntil) {
    ctx.waitUntil(work);
    return true;
  }
  return work;
}
