import {
  baseUrl,
  buildAccessHeaders,
  buildForwardHeaders,
  isAllowedAnalyticsOrigin,
  parseAllowedOrigins,
  parseBeaconPayload,
} from "./origin.ts";
import type {
  CollectProxyOptions,
  EventProxyOptions,
  ExecutionContextLike,
  RotameterEnv,
} from "./types.ts";

export const MAX_BEACON_BODY_BYTES = 4096;

function offlineScriptResponse(): Response {
  return new Response("/* rotameter offline */\n", {
    status: 200,
    headers: {
      "Content-Type": "application/javascript; charset=utf-8",
      "Cache-Control": "no-store",
      "X-Content-Type-Options": "nosniff",
    },
  });
}

export async function handleCollectScriptProxy(
  _request: Request,
  env: RotameterEnv,
  opts: CollectProxyOptions = {},
): Promise<Response> {
  const upstream = env.ROTAMETER_URL?.trim();
  if (!upstream) return offlineScriptResponse();

  const fetcher = opts.fetcher ?? fetch;
  const timeoutMs = opts.timeoutMs ?? 2000;
  try {
    const res = await fetcher(`${baseUrl(upstream)}/v1/collect.js`, {
      method: "GET",
      headers: buildAccessHeaders(env),
      signal: AbortSignal.timeout(timeoutMs),
    });
    if (!res.ok) return offlineScriptResponse();
    const script = await res.text();
    return new Response(script, {
      status: 200,
      headers: {
        "Content-Type": "application/javascript; charset=utf-8",
        "Cache-Control": opts.cacheControl ?? "public, max-age=86400",
        "X-Content-Type-Options": "nosniff",
      },
    });
  } catch {
    return offlineScriptResponse();
  }
}

export async function forwardAnalyticsEvent(
  request: Request,
  rawBody: string,
  env: RotameterEnv,
  opts: EventProxyOptions = {},
): Promise<boolean> {
  const upstream = env.ROTAMETER_URL?.trim();
  const ingestKey = env.ROTAMETER_INGEST_KEY?.trim();
  const siteId = env.ROTAMETER_SITE?.trim();
  if (!upstream || !ingestKey || !siteId) return false;

  const payload = parseBeaconPayload(rawBody, siteId);
  if (!payload) return false;

  const allowed = parseAllowedOrigins(env.ROTAMETER_ALLOWED_ORIGINS);
  if (!isAllowedAnalyticsOrigin(request, allowed, payload.url)) return false;

  const fetcher = opts.fetcher ?? fetch;
  const timeoutMs = opts.timeoutMs ?? 1500;
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
}

export async function handleEventProxy(
  request: Request,
  env: RotameterEnv,
  ctx?: ExecutionContextLike,
  opts: EventProxyOptions = {},
): Promise<Response> {
  const maxBytes = opts.maxBodyBytes ?? MAX_BEACON_BODY_BYTES;
  const rawBody = await request.text();
  if (new TextEncoder().encode(rawBody).byteLength > maxBytes) {
    return new Response(null, { status: 413 });
  }

  const siteId = env.ROTAMETER_SITE?.trim();
  if (!siteId) return new Response(null, { status: 204 });

  const payload = parseBeaconPayload(rawBody, siteId);
  const allowed = parseAllowedOrigins(env.ROTAMETER_ALLOWED_ORIGINS);
  if (!payload || !isAllowedAnalyticsOrigin(request, allowed, payload.url)) {
    return new Response(null, { status: 204 });
  }

  const work = forwardAnalyticsEvent(request, rawBody, env, opts);
  if (ctx?.waitUntil) {
    ctx.waitUntil(work);
  } else {
    await work;
  }

  return new Response(null, { status: 202 });
}
