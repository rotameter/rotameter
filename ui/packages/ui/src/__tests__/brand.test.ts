import { describe, expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { BRAND_ICON_NAMES } from "../primitives/BrandIcon";

const BRAND_DIR = new URL("../../../../../assets/brand/", import.meta.url).pathname;

const EXPECTED_PUBLIC_ASSETS = [
  "favicon.ico",
  "favicon.svg",
  "favicon-16x16.png",
  "favicon-32x32.png",
  "apple-touch-icon.png",
  "icon-192.png",
  "icon-512.png",
  "site.webmanifest",
  "icons-sprite.svg",
  "logo-light.svg",
  "logo-dark.svg",
];

describe("brand assets & sprite integrity", () => {
  test("every BrandIconName exists in icons-sprite.svg", () => {
    const sprite = readFileSync(join(BRAND_DIR, "icons-sprite.svg"), "utf8");
    for (const name of BRAND_ICON_NAMES) {
      expect(sprite).toContain(`id="${name}"`);
    }
  });

  test("all allowlisted brand public assets exist under assets/brand", () => {
    for (const file of EXPECTED_PUBLIC_ASSETS) {
      expect(existsSync(join(BRAND_DIR, file))).toBe(true);
    }
  });
});
