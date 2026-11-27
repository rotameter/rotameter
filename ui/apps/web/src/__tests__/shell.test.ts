import { describe, expect, it } from "bun:test";
import {
  parseFiltersParam,
  searchToQueryState,
  serializeFiltersParam,
  validateDashboardSearch,
} from "../searchParams";
import { DEFAULT_PREFERENCES, loadPreferences, resolveTheme, savePreferences } from "../preferences";

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
