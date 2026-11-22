# `@rotameter/astro-cloudflare`

Zero-dependency TypeScript integration package for first-party Astro + Cloudflare Workers proxying and SSR tracking behind Cloudflare Tunnel & Access.

## Worker Proxy Routes (Astro)

**`src/pages/api/r.js.ts`** (edge-cached first-party `/v1/collect.js` proxy):

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

## Browser Script Tag

```html
<script
  defer
  src="/api/r.js"
  data-site="colinwolff.dev"
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
- `ROTAMETER_ALLOWED_ORIGINS` — comma-separated production hosts (`colinwolff.dev,www.colinwolff.dev`)
- `CF_ACCESS_CLIENT_ID` / `CF_ACCESS_CLIENT_SECRET` (also supports `ROTAMETER_ACCESS_CLIENT_ID` / `ROTAMETER_ACCESS_CLIENT_SECRET`)
