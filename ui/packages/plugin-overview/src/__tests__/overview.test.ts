import { describe, expect, test } from "bun:test";
import { buildUplotData, computeDelta, formatCount, formatDurationMs, formatPercent } from "../format";
import overviewManifest from "../index";

describe("@rotameter/plugin-overview formatting & uPlot alignment", () => {
  test("formats counts, percentages, and durations cleanly", () => {
    expect(formatCount(12450)).toBe("12,450");
    expect(formatPercent(0.427)).toBe("42.7%");
    expect(formatDurationMs(45_000)).toBe("45s");
    expect(formatDurationMs(135_000)).toBe("2m 15s");
    expect(formatDurationMs(3_840_000)).toBe("1h 04m");
  });

  test("computes period deltas with inverted polarity for bounce rate", () => {
    const up = computeDelta(120, 100);
    expect(up?.label).toBe("+20.0%");
    expect(up?.tone).toBe("ok");

    const bounceDrop = computeDelta(0.3, 0.5, true);
    expect(bounceDrop?.label).toBe("-40.0%");
    expect(bounceDrop?.tone).toBe("ok");

    const bounceRise = computeDelta(0.6, 0.4, true);
    expect(bounceRise?.tone).toBe("crit");
  });

  test("aligns timeseries buckets into uPlot arrays", () => {
    const [xs, visitors, pageviews, visits] = buildUplotData([
      { bucket: "2026-11-26T00:00:00Z", visitors: 12, pageviews: 30, visits: 15 },
      { bucket: "2026-11-26T01:00:00Z", visitors: 20, pageviews: 42, visits: 24 },
    ]);
    expect(xs.length).toBe(2);
    expect((xs[1] ?? 0) - (xs[0] ?? 0)).toBe(3600);
    expect(visitors).toEqual([12, 20]);
    expect(pageviews).toEqual([30, 42]);
    expect(visits).toEqual([15, 24]);
  });

  test("exports a read-scoped overview PluginManifest", () => {
    expect(overviewManifest.id).toBe("overview");
    expect(overviewManifest.required_scopes).toEqual(["read:aggregates"]);
    expect(overviewManifest.requires_query).toBe(true);
    expect(overviewManifest.routes?.map((r) => r.path)).toEqual(["overview"]);
  });
});
