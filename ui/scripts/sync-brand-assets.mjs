import { copyFileSync, existsSync, mkdirSync } from "node:fs";
import { join } from "node:path";

const UI_ROOT = new URL("..", import.meta.url).pathname;
const SRC_DIR = join(UI_ROOT, "..", "assets", "brand");
const DEST_DIR = join(UI_ROOT, "apps", "web", "public");

export const BRAND_PUBLIC_FILES = [
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

mkdirSync(DEST_DIR, { recursive: true });

for (const name of BRAND_PUBLIC_FILES) {
  const src = join(SRC_DIR, name);
  if (!existsSync(src)) {
    console.error(`Missing brand asset: ${src}`);
    process.exit(1);
  }
  copyFileSync(src, join(DEST_DIR, name));
}

console.log(`brand assets synced: ${BRAND_PUBLIC_FILES.length} files -> apps/web/public/`);
