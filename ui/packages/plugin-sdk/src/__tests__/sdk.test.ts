import { describe, expect, it } from "bun:test";
import {
  isPluginEnabled,
  RotameterApiClient,
  type CapabilitiesResponse,
} from "../index";

describe("RotameterApiClient", () => {
  it("injects X-CSRF-Token on mutating calls and forwards AbortSignal", async () => {
    const seen: Array<{ url: string; method: string; csrf: string | null; hasSignal: boolean }> = [];
    const client = new RotameterApiClient({
      query_base_url: "https://analytics.example.com/",
      getCsrfToken: () => "csrf_tok_123",
      fetchImpl: (async (input: RequestInfo | URL, init?: RequestInit) => {
        const headers = new Headers(init?.headers);
        seen.push({
          url: String(input),
          method: init?.method ?? "GET",
          csrf: headers.get("X-CSRF-Token"),
          hasSignal: Boolean(init?.signal),
        });
        return new Response(JSON.stringify({ authenticated: false }), {
          status: 200,
          headers: { "Content-Type": "application/json" },
        });
      }) as typeof fetch,
    });

    const controller = new AbortController();
    await client.capabilities(controller.signal);
    await client.logout(controller.signal);

    expect(seen[0]).toEqual({
      url: "https://analytics.example.com/v1/capabilities",
      method: "GET",
      csrf: null,
      hasSignal: true,
    });
    expect(seen[1]).toEqual({
      url: "https://analytics.example.com/v1/auth/logout",
      method: "POST",
      csrf: "csrf_tok_123",
      hasSignal: true,
    });
  });
});

describe("PluginRegistry scope & role gating", () => {
  const ingestCaps: CapabilitiesResponse = {
    topology: "single",
    node_role: "ingest",
    features: { ui: true, arrow_export: false, oidc: false, streams: false },
    auth_methods: ["bearer"],
    endpoints: { query: "/v1", stream: "/v1" },
  };
  const allCaps: CapabilitiesResponse = { ...ingestCaps, node_role: "all" };

  it("gates query plugins on ingest nodes and admin plugins for read scopes", () => {
    expect(
      isPluginEnabled({ required_scopes: ["read:aggregates"], requires_query: true }, ingestCaps, "admin"),
    ).toBe(false);
    expect(
      isPluginEnabled({ required_scopes: ["read:aggregates"], requires_query: true }, allCaps, "read"),
    ).toBe(true);
    expect(
      isPluginEnabled({ required_scopes: ["admin:sites", "admin:keys"] }, allCaps, "read"),
    ).toBe(false);
    expect(
      isPluginEnabled({ required_scopes: ["admin:sites", "admin:keys"] }, allCaps, "admin"),
    ).toBe(true);
  });
});
