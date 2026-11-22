import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { handleCollectScriptProxy, handleEventProxy, trackServerHit } from "../integrations/astro-cloudflare/src/index.ts";

const bin = process.env.ROTAMETER_BIN || "./target/debug/rotameter";
const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "rotameter-astro-e2e-"));
const db = path.join(tmpDir, "e2e.db");
const admin = "rm_adm_astro_e2e_token_123456";

const port = await new Promise((resolve) => {
  const s = net.createServer();
  s.listen(0, "127.0.0.1", () => {
    const addr = s.address();
    s.close(() => resolve(addr.port));
  });
});

const srv = spawn(bin, ["serve", "--bind", `127.0.0.1:${port}`, "--db", db, "--bootstrap-admin-key", admin, "--no-self-site"], { stdio: "ignore" });
const base = `http://127.0.0.1:${port}`;

try {
  for (let i = 0; i < 60; i++) {
    try {
      const r = await fetch(`${base}/v1/health`);
      if (r.ok) break;
    } catch {}
    await new Promise((r) => setTimeout(r, 50));
  }

  const admHdr = { authorization: `Bearer ${admin}`, "content-type": "application/json" };
  await fetch(`${base}/v1/admin/sites`, { method: "POST", headers: admHdr, body: JSON.stringify({ slug: "astro-e2e.dev" }) });
  const ingKey = await (await fetch(`${base}/v1/admin/keys`, {
    method: "POST", headers: admHdr, body: JSON.stringify({ scope: "ingest", site: "astro-e2e.dev", label: "worker" }),
  })).json();
  const rdKey = await (await fetch(`${base}/v1/admin/keys`, {
    method: "POST", headers: admHdr, body: JSON.stringify({ scope: "read", site: "astro-e2e.dev", label: "ssr" }),
  })).json();

  const env = {
    ROTAMETER_URL: base,
    ROTAMETER_SITE: "astro-e2e.dev",
    ROTAMETER_INGEST_KEY: ingKey.token,
    ROTAMETER_ALLOWED_ORIGINS: "astro-e2e.dev",
  };
  const scriptRes = await handleCollectScriptProxy(new Request("https://astro-e2e.dev/api/r.js"), env);
  assert.equal(scriptRes.status, 200);
  assert.match(await scriptRes.text(), /rotameter_ignore/);

  const tasks = [];
  const proxyRes = await handleEventProxy(
    new Request("https://astro-e2e.dev/api/e", {
      method: "POST",
      headers: {
        origin: "https://astro-e2e.dev",
        "user-agent": "Mozilla/5.0 Chrome/131.0",
        "cf-connecting-ip": "203.0.113.55",
        "cf-ipcountry": "US",
        "cf-region-code": "OR",
      },
      body: JSON.stringify({ url: "https://astro-e2e.dev/projects/rotameter" }),
    }),
    env,
    { waitUntil: (p) => tasks.push(p) },
  );
  assert.equal(proxyRes.status, 202);
  await trackServerHit(
    new Request("https://astro-e2e.dev/status", { headers: { "cf-connecting-ip": "203.0.113.56", "cf-ipcountry": "US" } }),
    env,
    { waitUntil: (p) => tasks.push(p) },
  );
  await Promise.all(tasks);

  let q;
  for (let i = 0; i < 40; i++) {
    const r = await fetch(`${base}/v1/query`, {
      method: "POST",
      headers: { authorization: `Bearer ${rdKey.token}`, "content-type": "application/json" },
      body: JSON.stringify({ site_id: "astro-e2e.dev", range: { preset: "24h" }, metrics: ["visitors", "pageviews"], group_by: ["country"] }),
    });
    q = await r.json();
    if (q.summary?.pageviews >= 2) break;
    await new Promise((res) => setTimeout(res, 100));
  }
  assert.equal(q.summary.visitors >= 1, true);
  assert.equal(q.summary.pageviews, 2);
  assert.equal(q.rows[0].key.country, "US");
  console.log("OK: Astro/Cloudflare Workers E2E proxy + SSR hit verified.");
} finally {
  srv.kill("SIGTERM");
  await new Promise((resolve) => srv.once("exit", resolve));
  fs.rmSync(tmpDir, { recursive: true, force: true });
}
