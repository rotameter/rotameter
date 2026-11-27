import { describe, expect, test } from "bun:test";
import { computeBarRatio, parsePxValue, toggleDimensionFilter } from "../dimensions";
import breakdownsManifest from "../index";

describe("@rotameter/plugin-breakdowns helpers & manifest", () => {
  test("parses --density-table-row-h pixel strings safely", () => {
    expect(parsePxValue("26px")).toBe(26);
    expect(parsePxValue(" 42px ")).toBe(42);
    expect(parsePxValue("", 34)).toBe(34);
  });

  test("computes normalized proportion bar ratios", () => {
    expect(computeBarRatio(50, 200)).toBe(0.25);
    expect(computeBarRatio(0, 200)).toBe(0);
    expect(computeBarRatio(10, 0)).toBe(0);
  });

  test("toggles dimension filters immutably via updateQuery contract", () => {
    const first = toggleDimensionFilter([], "path", "/pricing");
    expect(first).toEqual([{ dimension: "path", op: "eq", value: "/pricing" }]);

    const replaced = toggleDimensionFilter(first, "path", "/docs");
    expect(replaced).toEqual([{ dimension: "path", op: "eq", value: "/docs" }]);

    const cleared = toggleDimensionFilter(replaced, "path", "/docs");
    expect(cleared).toEqual([]);
  });

  test("exports a read-scoped breakdowns PluginManifest", () => {
    expect(breakdownsManifest.id).toBe("breakdowns");
    expect(breakdownsManifest.required_scopes).toEqual(["read:aggregates"]);
    expect(breakdownsManifest.requires_query).toBe(true);
    expect(breakdownsManifest.routes?.map((r) => r.path)).toEqual(["breakdowns"]);
  });
});
