import { describe, expect, test } from "bun:test";
import { formatMinuteLabel, normalizePulseBars } from "../pulse";
import realtimeManifest from "../index";

describe("@rotameter/plugin-realtime pulse helpers & manifest", () => {
  test("formats UTC minute labels and normalizes bar ratios", () => {
    expect(formatMinuteLabel("2026-11-26T14:07:00Z")).toBe("14:07");
    const res = normalizePulseBars([
      { minute: "2026-11-26T14:05:00Z", events: 5 },
      { minute: "2026-11-26T14:06:00Z", events: 20 },
      { minute: "2026-11-26T14:07:00Z", events: 0 },
    ]);
    expect(res.maxEvents).toBe(20);
    expect(res.totalEvents).toBe(25);
    expect(res.bars.map((b) => b.ratio)).toEqual([0.25, 1, 0]);
  });

  test("exports a read-scoped realtime PluginManifest", () => {
    expect(realtimeManifest.id).toBe("realtime");
    expect(realtimeManifest.required_scopes).toEqual(["read:aggregates"]);
    expect(realtimeManifest.requires_query).toBe(true);
    expect(realtimeManifest.routes?.map((r) => r.path)).toEqual(["realtime"]);
  });
});
