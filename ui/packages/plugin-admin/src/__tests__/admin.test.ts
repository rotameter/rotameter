import { describe, expect, test } from "bun:test";
import {
  buildCollectSnippet,
  buildSitePayload,
  emptySiteForm,
  parseListInput,
  validateKeyForm,
} from "../forms";
import adminManifest from "../index";

describe("@rotameter/plugin-admin forms & manifest", () => {
  test("parses comma and newline separated lists with deduplication", () => {
    expect(parseListInput("https://a.dev,\nhttps://b.dev, https://a.dev\n")).toEqual([
      "https://a.dev",
      "https://b.dev",
    ]);
  });

  test("validates site create vs edit payloads cleanly", () => {
    const empty = emptySiteForm();
    expect(buildSitePayload(empty, "create")).toEqual({
      ok: false,
      error: "Site slug is required.",
    });

    const badRetention = buildSitePayload({ ...empty, slug: "demo.dev", raw_retention_days: "0" }, "create");
    expect(badRetention.ok).toBe(false);

    const created = buildSitePayload(
      { ...empty, slug: "demo.dev", raw_retention_days: "90", allowed_origins: "https://demo.dev" },
      "create",
    );
    expect(created).toEqual({
      ok: true,
      payload: {
        slug: "demo.dev",
        timezone: "UTC",
        salt_rotation: "daily",
        respect_gpc: false,
        allowed_origins: ["https://demo.dev"],
        allowed_query_keys: [],
        raw_retention_days: 90,
      },
    });

    const edited = buildSitePayload({ ...empty, slug: "demo.dev", display_name: "Demo" }, "edit");
    expect(edited.ok).toBe(true);
    if (edited.ok) {
      expect(edited.payload.slug).toBeUndefined();
      expect(edited.payload.display_name).toBe("Demo");
    }
  });

  test("validates scoped API key creation rules", () => {
    expect(validateKeyForm({ label: "reader", scope: "read", site: "" })).toEqual({
      ok: false,
      error: "Select a site for read/ingest API keys.",
    });
    expect(validateKeyForm({ label: "admin-key", scope: "admin", site: "site_123" })).toEqual({
      ok: true,
      payload: { scope: "admin", label: "admin-key" },
    });
    expect(validateKeyForm({ label: "x".repeat(129), scope: "admin", site: "" }).ok).toBe(false);
  });

  test("builds collect.js snippet stripping trailing origin slashes", () => {
    const snippet = buildCollectSnippet("https://analytics.example.com/", "site_abc");
    expect(snippet).toContain('src="https://analytics.example.com/v1/collect.js"');
    expect(snippet).toContain('data-site="site_abc"');
    expect(snippet).toContain('src="https://analytics.example.com/v1/pixel.gif?site=site_abc"');
  });

  test("exports admin-scoped PluginManifest on /admin", () => {
    expect(adminManifest.id).toBe("admin");
    expect(adminManifest.required_scopes).toEqual(["admin:sites", "admin:keys"]);
    expect(adminManifest.requires_query).toBeUndefined();
    expect(adminManifest.routes?.map((r) => r.path)).toEqual(["/admin"]);
  });
});
