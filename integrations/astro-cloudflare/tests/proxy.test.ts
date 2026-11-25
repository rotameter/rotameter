import assert from "node:assert/strict";
import test from "node:test";
import {
  handleCollectScriptProxy,
  handleEventProxy,
} from "../src/index.ts";
import type { RotameterEnv } from "../src/index.ts";

const PROD_ENV: RotameterEnv = {
  ROTAMETER_URL: "https://analytics.internal.example/",
  ROTAMETER_SITE: "colinwolff.dev",
  ROTAMETER_INGEST_KEY: "rm_ing_worker_key_123",
  ROTAMETER_ALLOWED_ORIGINS: "colinwolff.dev,www.colinwolff.dev",
  CF_ACCESS_CLIENT_ID: "cf-id-1",
  CF_ACCESS_CLIENT_SECRET: "cf-secret-1",
};

test("handleCollectScriptProxy proxies collector with Access headers and falls back offline", async () => {
  let seenUrl = "";
  let seenHeaders: Headers | undefined;
  const okRes = await handleCollectScriptProxy(
    new Request("https://colinwolff.dev/api/r.js"),
    PROD_ENV,
    {
      fetcher: async (input, init) => {
        seenUrl = String(input);
        seenHeaders = new Headers(init?.headers);
        return new Response("console.log('rm');", { status: 200 });
      },
    },
  );
  assert.equal(okRes.status, 200);
  assert.equal(seenUrl, "https://analytics.internal.example/v1/collect.js");
  assert.equal(seenHeaders?.get("CF-Access-Client-Id"), "cf-id-1");
  assert.equal(seenHeaders?.get("CF-Access-Client-Secret"), "cf-secret-1");
  assert.equal(okRes.headers.get("Cache-Control"), "public, max-age=86400");
  assert.equal(await okRes.text(), "console.log('rm');");

  const offlineRes = await handleCollectScriptProxy(
    new Request("https://colinwolff.dev/api/r.js"),
    PROD_ENV,
    { fetcher: async () => new Response("down", { status: 503 }) },
  );
  assert.equal(offlineRes.status, 200);
  assert.equal(offlineRes.headers.get("Cache-Control"), "no-store");
  assert.match(await offlineRes.text(), /rotameter offline/);

  const unconfigured = await handleCollectScriptProxy(
    new Request("https://colinwolff.dev/api/r.js"),
    {},
  );
  assert.equal(unconfigured.status, 200);
});

test("handleEventProxy enforces size cap, drops preview hosts, and relays via waitUntil", async () => {
  const oversize = new Request("https://colinwolff.dev/api/e", {
    method: "POST",
    body: JSON.stringify({ url: `https://colinwolff.dev/${"a".repeat(5000)}` }),
  });
  assert.equal((await handleEventProxy(oversize, PROD_ENV)).status, 413);

  for (const host of ["localhost:4321", "preview.colinwolff.workers.dev", "evil.example"]) {
    const req = new Request(`https://${host}/api/e`, {
      method: "POST",
      headers: { origin: `https://${host}` },
      body: JSON.stringify({ url: `https://${host}/posts/test` }),
    });
    assert.equal((await handleEventProxy(req, PROD_ENV)).status, 204);
  }

  let forwarded: { url: string; headers: Headers; body: Record<string, unknown> } | undefined;
  const tasks: Promise<unknown>[] = [];
  const validReq = new Request("https://colinwolff.dev/api/e", {
    method: "POST",
    headers: {
      origin: "https://colinwolff.dev",
      "user-agent": "Mozilla/5.0 Chrome/131.0",
      "sec-gpc": "1",
      "cf-connecting-ip": "203.0.113.42",
      "cf-ipcountry": "US",
      "cf-region-code": "OR",
    },
    body: JSON.stringify({
      event: "signup_clicked",
      url: "https://colinwolff.dev/projects/rotameter",
      viewport_width: 1440,
      props: { plan: "pro", ignored: { nested: true } },
    }),
  });

  const envFallback: RotameterEnv = {
    ROTAMETER_URL: PROD_ENV.ROTAMETER_URL,
    ROTAMETER_SITE: PROD_ENV.ROTAMETER_SITE,
    ROTAMETER_INGEST_KEY: PROD_ENV.ROTAMETER_INGEST_KEY,
    ROTAMETER_ALLOWED_ORIGINS: PROD_ENV.ROTAMETER_ALLOWED_ORIGINS,
    ROTAMETER_ACCESS_CLIENT_ID: "fallback-id",
    ROTAMETER_ACCESS_CLIENT_SECRET: "fallback-secret",
  };
  const res = await handleEventProxy(
    validReq,
    envFallback,
    { waitUntil: (p) => tasks.push(p) },
    {
      fetcher: async (input, init) => {
        forwarded = {
          url: String(input),
          headers: new Headers(init?.headers),
          body: JSON.parse(String(init?.body)) as Record<string, unknown>,
        };
        return new Response(null, { status: 202 });
      },
    },
  );
  assert.equal(res.status, 202);
  assert.equal(tasks.length, 1);
  await Promise.all(tasks);

  assert.equal(forwarded?.url, "https://analytics.internal.example/v1/events");
  assert.equal(forwarded?.headers.get("Authorization"), "Bearer rm_ing_worker_key_123");
  assert.equal(forwarded?.headers.get("CF-Access-Client-Id"), "fallback-id");
  assert.equal(forwarded?.headers.get("CF-Access-Client-Secret"), "fallback-secret");
  assert.equal(forwarded?.headers.get("X-Rotameter-Client-IP"), "203.0.113.42");
  assert.equal(forwarded?.headers.get("X-Rotameter-Country"), "US");
  assert.equal(forwarded?.headers.get("X-Rotameter-Region"), "OR");
  assert.equal(forwarded?.headers.get("Sec-GPC"), "1");
  assert.deepEqual(forwarded?.body.props, { plan: "pro" });
});

test("handleEventProxy allows explicitly listed preview hosts while still blocking unlisted previews and localhost", async () => {
  const stagingEnv: RotameterEnv = {
    ...PROD_ENV,
    ROTAMETER_SITE: "staging.colinwolff.dev",
    ROTAMETER_ALLOWED_ORIGINS: "staging.colinwolff.workers.dev,staging.colinwolff.pages.dev,localhost",
  };

  for (const allowedHost of ["staging.colinwolff.workers.dev", "staging.colinwolff.pages.dev"]) {
    let forwardedUrl = "";
    const req = new Request(`https://${allowedHost}/api/e`, {
      method: "POST",
      headers: { origin: `https://${allowedHost}` },
      body: JSON.stringify({ url: `https://${allowedHost}/posts/preview` }),
    });
    const res = await handleEventProxy(req, stagingEnv, undefined, {
      fetcher: async (input) => {
        forwardedUrl = String(input);
        return new Response(null, { status: 202 });
      },
    });
    assert.equal(res.status, 202);
    assert.equal(forwardedUrl, "https://analytics.internal.example/v1/events");
  }

  for (const blockedHost of ["pr-99.colinwolff.workers.dev", "pr-99.colinwolff.pages.dev", "localhost:4321"]) {
    const req = new Request(`https://${blockedHost}/api/e`, {
      method: "POST",
      headers: { origin: `https://${blockedHost}` },
      body: JSON.stringify({ url: `https://${blockedHost}/posts/preview` }),
    });
    assert.equal((await handleEventProxy(req, stagingEnv)).status, 204);
  }

  const localAllowedRes = await handleEventProxy(
    new Request("http://localhost:4321/api/e", {
      method: "POST",
      headers: { origin: "http://localhost:4321" },
      body: JSON.stringify({ url: "http://localhost:4321/posts/local" }),
    }),
    stagingEnv,
    undefined,
    {
      allowLocal: true,
      fetcher: async () => new Response(null, { status: 202 }),
    },
  );
  assert.equal(localAllowedRes.status, 202);

  let precedenceHeaders: Headers | undefined;
  const precedenceEnv: RotameterEnv = {
    ...PROD_ENV,
    CF_ACCESS_CLIENT_ID: "generic-cf-id",
    CF_ACCESS_CLIENT_SECRET: "generic-cf-secret",
    ROTAMETER_ACCESS_CLIENT_ID: "rotameter-specific-id",
    ROTAMETER_ACCESS_CLIENT_SECRET: "rotameter-specific-secret",
  };
  const precedenceRes = await handleEventProxy(
    new Request("https://colinwolff.dev/api/e", {
      method: "POST",
      headers: { origin: "https://colinwolff.dev" },
      body: JSON.stringify({ url: "https://colinwolff.dev/posts/precedence" }),
    }),
    precedenceEnv,
    undefined,
    {
      fetcher: async (_input, init) => {
        precedenceHeaders = new Headers(init?.headers);
        return new Response(null, { status: 202 });
      },
    },
  );
  assert.equal(precedenceRes.status, 202);
  assert.equal(precedenceHeaders?.get("CF-Access-Client-Id"), "rotameter-specific-id");
  assert.equal(precedenceHeaders?.get("CF-Access-Client-Secret"), "rotameter-specific-secret");
});
