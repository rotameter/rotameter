import assert from "node:assert/strict";
import test from "node:test";
import { createCollectScriptRoute, createEventProxyRoute } from "../src/astro.ts";
import { trackServerHit } from "../src/index.ts";
import type { RotameterEnv } from "../src/index.ts";

const PROD_ENV: RotameterEnv = {
  ROTAMETER_URL: "https://analytics.internal.example",
  ROTAMETER_SITE: "colinwolff.dev",
  ROTAMETER_INGEST_KEY: "rm_ing_ssr_key_456",
  ROTAMETER_ALLOWED_ORIGINS: "colinwolff.dev",
  CF_ACCESS_CLIENT_ID: "cf-id",
  CF_ACCESS_CLIENT_SECRET: "cf-sec",
};

test("trackServerHit sends SSR hits via waitUntil, drops previews, and never throws", async () => {
  const previewDropped = await trackServerHit(
    new Request("https://branch.colinwolff.workers.dev/status"),
    PROD_ENV,
  );
  assert.equal(previewDropped, false);

  const tasks: Promise<unknown>[] = [];
  let sentBody: Record<string, unknown> | undefined;
  const req = new Request("https://colinwolff.dev/missing-page", {
    headers: {
      referer: "https://news.ycombinator.com/",
      "cf-connecting-ip": "198.51.100.10",
      "cf-ipcountry": "DE",
    },
  });

  const scheduled = await trackServerHit(
    req,
    PROD_ENV,
    { waitUntil: (p) => tasks.push(p) },
    {
      event: "not_found",
      props: { route: "404" },
      fetcher: async (_url, init) => {
        sentBody = JSON.parse(String(init?.body)) as Record<string, unknown>;
        return new Response(null, { status: 202 });
      },
    },
  );
  assert.equal(scheduled, true);
  assert.equal(tasks.length, 1);
  await Promise.all(tasks);
  assert.equal(sentBody?.event, "not_found");
  assert.equal(sentBody?.url, "https://colinwolff.dev/missing-page");
  assert.equal(sentBody?.referrer, "https://news.ycombinator.com/");
  assert.deepEqual(sentBody?.props, { route: "404" });

  const failed = await trackServerHit(req, PROD_ENV, undefined, {
    fetcher: async () => {
      throw new Error("tunnel offline");
    },
  });
  assert.equal(failed, false);
});

test("Astro route adapters resolve locals.runtime.env and locals.cfContext", async () => {
  const getScript = createCollectScriptRoute({
    fetcher: async () => new Response("/* ok */", { status: 200 }),
  });
  const scriptResp = await getScript({
    request: new Request("https://colinwolff.dev/api/r.js"),
    locals: { runtime: { env: PROD_ENV } },
  });
  assert.equal(scriptResp.status, 200);
  assert.equal(await scriptResp.text(), "/* ok */");

  const tasks: Promise<unknown>[] = [];
  const postEvent = createEventProxyRoute({
    fetcher: async () => new Response(null, { status: 202 }),
  });
  const eventResp = await postEvent({
    request: new Request("https://colinwolff.dev/api/e", {
      method: "POST",
      headers: { origin: "https://colinwolff.dev" },
      body: JSON.stringify({ url: "https://colinwolff.dev/analytics" }),
    }),
    locals: {
      runtime: { env: PROD_ENV },
      cfContext: { waitUntil: (p) => tasks.push(p) },
    },
  });
  assert.equal(eventResp.status, 202);
  assert.equal(tasks.length, 1);
  await Promise.all(tasks);
});

test("trackServerHit honors explicitly listed preview hosts while blocking unlisted previews and localhost", async () => {
  const stagingEnv: RotameterEnv = {
    ...PROD_ENV,
    ROTAMETER_ALLOWED_ORIGINS: "staging.colinwolff.workers.dev,localhost",
  };

  let sentCount = 0;
  const ok = await trackServerHit(
    new Request("https://staging.colinwolff.workers.dev/status"),
    stagingEnv,
    undefined,
    {
      fetcher: async () => {
        sentCount += 1;
        return new Response(null, { status: 202 });
      },
    },
  );
  assert.equal(ok, true);
  assert.equal(sentCount, 1);

  const blockedPreview = await trackServerHit(
    new Request("https://branch.colinwolff.workers.dev/status"),
    stagingEnv,
  );
  assert.equal(blockedPreview, false);

  const blockedLocal = await trackServerHit(
    new Request("http://localhost:4321/status"),
    stagingEnv,
  );
  assert.equal(blockedLocal, false);

  const allowlistOnlyLocal = await trackServerHit(
    new Request("https://staging.colinwolff.workers.dev/status"),
    { ...PROD_ENV, ROTAMETER_ALLOWED_ORIGINS: "localhost" },
  );
  assert.equal(allowlistOnlyLocal, false);
});
