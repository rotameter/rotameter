# `@rotameter/astro-cloudflare`

Zero-dependency TypeScript integration package for first-party Astro + Cloudflare Workers proxying, CSP-safe bundled beacons, and SSR tracking behind Cloudflare Tunnel & Access.

## Option 1: Bundled CSP-Safe Beacon (Recommended for Astro)

Instead of serving `/api/r.js` on every pageview, bundle `@rotameter/astro-cloudflare/beacon` directly inside an Astro `<script>` module so Astro can generate deterministic CSP SHA-256 hashes with zero extra Worker invocations:

```astro
<script>
  import { initRotameterBeacon } from "@rotameter/astro-cloudflare/beacon";

  initRotameterBeacon({
    endpoint: "/api/e",
    spa: true,
    outbound: true,
    downloads: true,
  });
</script>
```

On `404.astro`, pass `notFound: true` or add `<meta name="rotameter-not-found" content="true" />` / `data-rotameter-not-found` on `<html>` or `<body>` (or emit a zero-JS SSR hit via `trackServerHit`). The beacon respects `localStorage.rotameter_ignore === "true"`, skips `localhost` by default, and exposes `window.rotameter(event, { props })`.

## Option 2: Worker Script & Event Proxy Routes (Astro)

**`src/pages/api/r.js.ts`** (optional edge-cached first-party `/v1/collect.js` proxy when not bundling `@rotameter/astro-cloudflare/beacon`):

```ts
import type { APIRoute } from "astro";
import { env } from "cloudflare:workers";
import { handleCollectScriptProxy } from "@rotameter/astro-cloudflare";

export const prerender = false;

export const GET: APIRoute = ({ request }) => handleCollectScriptProxy(request, env);
```

**`src/pages/api/e.ts`** (`202 Accepted` ingest relay inside `waitUntil`):

```ts
import type { APIRoute } from "astro";
import { env } from "cloudflare:workers";
import { handleEventProxy } from "@rotameter/astro-cloudflare";

export const prerender = false;

export const POST: APIRoute = ({ request, locals }) =>
  handleEventProxy(request, env, locals.cfContext);
```

If using `/api/r.js`, include the script tag:

```html
<script
  defer
  src="/api/r.js"
  data-site="example.com"
  data-endpoint="/api/e"
></script>
```

## Pure SSR Hits (`trackServerHit`)

For zero-JS SSR routes (`/status`, `/rss.xml`, `404.astro`):

```ts
import { trackServerHit } from "@rotameter/astro-cloudflare";

await trackServerHit(Astro.request, env, Astro.locals.cfContext, {
  event: "not_found",
});
```

## Required Worker Environment Variables

- `ROTAMETER_URL` — private tunnel URL (e.g., `https://rotameter.internal.example`)
- `ROTAMETER_SITE` — target `site_...` UID or slug
- `ROTAMETER_INGEST_KEY` — scoped `rm_ing_...` Bearer token
- `ROTAMETER_ALLOWED_ORIGINS` — comma-separated exact hostnames (`example.com,www.example.com`). Explicitly listed staging/preview hosts (e.g., `staging.example.workers.dev` or `preview.example.pages.dev`) are honored while unlisted `*.workers.dev`/`*.pages.dev` previews and `localhost` remain dropped.
- `CF_ACCESS_CLIENT_ID` / `CF_ACCESS_CLIENT_SECRET` (also supports `ROTAMETER_ACCESS_CLIENT_ID` / `ROTAMETER_ACCESS_CLIENT_SECRET`)
