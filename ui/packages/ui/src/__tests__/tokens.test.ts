import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";

function parseBlockVars(css: string, headerPattern: RegExp): Record<string, string> {
  const match = headerPattern.exec(css);
  if (!match?.[1]) {
    throw new Error(`Missing CSS block for ${headerPattern}`);
  }
  const vars: Record<string, string> = {};
  for (const line of match[1].split("\n")) {
    const m = /^\s*(--[a-z0-9-]+)\s*:\s*([^;]+);/i.exec(line);
    if (m?.[1] && m?.[2]) {
      vars[m[1]] = m[2].trim();
    }
  }
  return vars;
}

function hexToRgb(hex: string): [number, number, number] {
  const clean = hex.replace(/^#/, "");
  if (clean.length !== 6) {
    throw new Error(`Expected 6-digit hex color, got ${hex}`);
  }
  return [
    Number.parseInt(clean.slice(0, 2), 16),
    Number.parseInt(clean.slice(2, 4), 16),
    Number.parseInt(clean.slice(4, 6), 16),
  ];
}

function relativeLuminance(hex: string): number {
  const channels = hexToRgb(hex).map((v) => {
    const srgb = v / 255;
    return srgb <= 0.03928 ? srgb / 12.92 : Math.pow((srgb + 0.055) / 1.055, 2.4);
  });
  return 0.2126 * (channels[0] ?? 0) + 0.7152 * (channels[1] ?? 0) + 0.0722 * (channels[2] ?? 0);
}

function contrastRatio(fgHex: string, bgHex: string): number {
  const l1 = relativeLuminance(fgHex);
  const l2 = relativeLuminance(bgHex);
  const lighter = Math.max(l1, l2);
  const darker = Math.min(l1, l2);
  return (lighter + 0.05) / (darker + 0.05);
}

describe("WCAG 2.2 AA & AAA design tokens", () => {
  const css = readFileSync(new URL("../tokens.css", import.meta.url), "utf8");
  const light = parseBlockVars(css, /:root,\s*:root\[data-theme="light"\]\s*\{([\s\S]*?)\}/);
  const dark = parseBlockVars(css, /:root\[data-theme="dark"\]\s*\{([\s\S]*?)\}/);
  const rootBase = parseBlockVars(css, /:root\s*\{([\s\S]*?)\}/);
  const compact = parseBlockVars(css, /\[data-density="compact"\]\s*\{([\s\S]*?)\}/);
  const standard = parseBlockVars(css, /:root,\s*\[data-density="standard"\]\s*\{([\s\S]*?)\}/);
  const comfortable = parseBlockVars(css, /\[data-density="comfortable"\]\s*\{([\s\S]*?)\}/);

  for (const [themeName, tokens] of [
    ["light", light],
    ["dark", dark],
  ] as const) {
    test(`${themeName} theme meets WCAG AAA primary numerals and AA UI contrast`, () => {
      const surface = tokens["--bg-surface"]!;
      const canvas = tokens["--bg-canvas"]!;
      const elevated = tokens["--bg-elevated"]!;

      expect(contrastRatio(tokens["--fg-primary"]!, surface)).toBeGreaterThanOrEqual(7.0);

      for (const fgKey of ["--fg-primary", "--fg-secondary", "--fg-muted"] as const) {
        for (const bg of [canvas, surface, elevated]) {
          expect(contrastRatio(tokens[fgKey]!, bg)).toBeGreaterThanOrEqual(4.5);
        }
      }

      expect(contrastRatio(tokens["--accent-fg"]!, tokens["--accent-primary"]!)).toBeGreaterThanOrEqual(4.5);

      for (const sigKey of ["--signal-ok", "--signal-warn", "--signal-crit"] as const) {
        expect(contrastRatio(tokens[sigKey]!, surface)).toBeGreaterThanOrEqual(4.5);
      }

      expect(contrastRatio(tokens["--focus-ring"]!, surface)).toBeGreaterThanOrEqual(3.0);
      expect(contrastRatio(tokens["--focus-ring"]!, canvas)).toBeGreaterThanOrEqual(3.0);
    });
  }

  test("WCAG AA typography scale stays constant across densities while spacing/heights scale", () => {
    const xs = Number.parseFloat(rootBase["--text-xs"] ?? "0");
    const sm = Number.parseFloat(rootBase["--text-sm"] ?? "0");
    const base = Number.parseFloat(rootBase["--text-base"] ?? "0");
    const md = Number.parseFloat(rootBase["--text-md"] ?? "0");
    const lg = Number.parseFloat(rootBase["--text-lg"] ?? "0");
    const kpi = Number.parseFloat(rootBase["--text-kpi"] ?? "0");

    expect(base).toBeGreaterThanOrEqual(1);
    expect(xs).toBeLessThan(sm);
    expect(sm).toBeLessThan(base);
    expect(base).toBeLessThan(md);
    expect(md).toBeLessThan(lg);
    expect(lg).toBeLessThan(kpi);

    for (const density of [compact, standard, comfortable]) {
      expect(density["--density-font-body"]).toBeUndefined();
      expect(density["--density-kpi"]).toBeUndefined();
    }

    for (const token of ["--density-grid-gap", "--density-control-h", "--density-table-row-h"] as const) {
      const cVal = Number.parseFloat(compact[token] ?? "0");
      const sVal = Number.parseFloat(standard[token] ?? "0");
      const bVal = Number.parseFloat(comfortable[token] ?? "0");
      expect(cVal).toBeLessThan(sVal);
      expect(sVal).toBeLessThan(bVal);
    }

    expect(Number.parseFloat(compact["--density-control-h"] ?? "0")).toBeGreaterThanOrEqual(24);
    expect(Number.parseFloat(compact["--density-table-row-h"] ?? "0")).toBeGreaterThanOrEqual(24);
    expect(css).toContain("outline: 2px solid var(--focus-ring)");
    expect(css).toContain("outline-offset: 2px");
  });
});
