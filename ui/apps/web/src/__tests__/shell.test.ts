import { describe, expect, it } from "bun:test";
import {
  parseFiltersParam,
  searchToQueryState,
  serializeFiltersParam,
  validateDashboardSearch,
} from "../searchParams";
import {
  DEFAULT_PREFERENCES,
  loadPreferences,
  resolveTheme,
  savePreferences,
} from "../preferences";
import { mountSelfSiteCollector } from "../selfSite";

describe("URL search param validation & filter serialization", () => {
  it("parses and round-trips canonical filter expressions", () => {
    const parsed = parseFiltersParam("country:US,path:/blog/*,source:!reddit.com,browser:~Firefox");
    expect(parsed).toEqual([
      { dimension: "country", op: "eq", value: "US" },
      { dimension: "path", op: "prefix", value: "/blog/" },
      { dimension: "source", op: "neq", value: "reddit.com" },
      { dimension: "browser", op: "contains", value: "Firefox" },
    ]);
    expect(serializeFiltersParam(parsed)).toBe(
      "country:US,path:/blog/*,source:!reddit.com,browser:~Firefox",
    );
  });

  it("validates defaults and maps into DashboardQueryState", () => {
    const validated = validateDashboardSearch({ range: "24h", compare: "prev", f: "country:US" });
    expect(validated).toEqual({
      range: "24h",
      interval: "hour",
      compare: true,
      f: "country:US",
    });
    expect(searchToQueryState("example.com", validated)).toEqual({
      siteId: "example.com",
      range: "24h",
      interval: "hour",
      compare: true,
      filters: [{ dimension: "country", op: "eq", value: "US" }],
    });
  });
});

describe("UI localStorage preferences", () => {
  it("saves, loads, and resolves theme safely", () => {
    const map = new Map<string, string>();
    const storage = {
      getItem: (k: string) => map.get(k) ?? null,
      setItem: (k: string, v: string) => {
        map.set(k, v);
      },
      removeItem: (k: string) => {
        map.delete(k);
      },
      clear: () => map.clear(),
      key: () => null,
      length: 0,
    } as Storage;

    expect(loadPreferences(storage)).toEqual(DEFAULT_PREFERENCES);
    savePreferences(
      { theme: "dark", density: "compact", sidebarCollapsed: true, streamPaused: false },
      storage,
    );
    expect(loadPreferences(storage)).toEqual({
      theme: "dark",
      density: "compact",
      sidebarCollapsed: true,
      streamPaused: false,
    });
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("light", true)).toBe("light");
  });
});

describe("self_site collector injection", () => {
  it("skips when self_site_id is absent and injects /v1/collect.js idempotently when present", () => {
    const appended: Array<{
      defer: boolean;
      src: string;
      attrs: Record<string, string>;
    }> = [];
    const fakeDoc = {
      querySelector: (selector: string) =>
        selector === "script[data-rotameter-self-site]" && appended.length > 0
          ? ({} as Element)
          : null,
      createElement: () => {
        const attrs: Record<string, string> = {};
        return {
          defer: false,
          src: "",
          attrs,
          setAttribute: (k: string, v: string) => {
            attrs[k] = v;
          },
        };
      },
      head: {
        appendChild: (el: { defer: boolean; src: string; attrs: Record<string, string> }) => {
          appended.push(el);
          return el;
        },
      },
    } as unknown as Document;

    expect(mountSelfSiteCollector(undefined, fakeDoc)).toBeNull();
    expect(appended).toHaveLength(0);

    const injected = mountSelfSiteCollector("site_self_123", fakeDoc);
    expect(injected).not.toBeNull();
    expect(appended).toHaveLength(1);
    expect(appended[0]?.defer).toBe(true);
    expect(appended[0]?.src).toBe("/v1/collect.js");
    expect(appended[0]?.attrs).toEqual({
      "data-site": "site_self_123",
      "data-endpoint": "/v1/events",
      "data-spa": "true",
      "data-rotameter-self-site": "true",
    });

    expect(mountSelfSiteCollector("site_self_123", fakeDoc)).toBeNull();
    expect(appended).toHaveLength(1);
  });
});
