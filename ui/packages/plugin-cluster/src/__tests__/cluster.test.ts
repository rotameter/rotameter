import { describe, expect, test } from "bun:test";
import { featureRows, formatUptime, queueTone } from "../health";
import clusterManifest from "../index";

describe("@rotameter/plugin-cluster health helpers & manifest", () => {
  test("formats uptimes cleanly across seconds, minutes, hours, and days", () => {
    expect(formatUptime(45)).toBe("45s");
    expect(formatUptime(125)).toBe("2m 05s");
    expect(formatUptime(7500)).toBe("2h 05m");
    expect(formatUptime(273600)).toBe("3d 04h");
  });

  test("classifies queue fill saturation tones at 70% and 90% thresholds", () => {
    expect(queueTone(0)).toBe("ok");
    expect(queueTone(69.9)).toBe("ok");
    expect(queueTone(70)).toBe("warn");
    expect(queueTone(89.9)).toBe("warn");
    expect(queueTone(90)).toBe("crit");
  });

  test("maps capability feature flags to labeled status rows", () => {
    const rows = featureRows({ ui: true, arrow_export: true, oidc: false, streams: false });
    expect(rows.map((r) => [r.key, r.enabled])).toEqual([
      ["ui", true],
      ["arrow_export", true],
      ["oidc", false],
      ["streams", false],
    ]);
  });

  test("exports cluster PluginManifest on /cluster without read:aggregates gating", () => {
    expect(clusterManifest.id).toBe("cluster");
    expect(clusterManifest.required_scopes).toEqual([]);
    expect(clusterManifest.requires_query).toBeUndefined();
    expect(clusterManifest.routes?.map((r) => r.path)).toEqual(["/cluster"]);
  });
});
