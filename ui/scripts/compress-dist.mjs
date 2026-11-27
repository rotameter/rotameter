import { readdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { extname, join, relative } from "node:path";
import { gzipSync } from "node:zlib";

const DIST_DIR = new URL("../dist", import.meta.url).pathname;
const MAX_EMBEDDED_BYTES = 1_572_864;
const MAX_INITIAL_GZIP_BYTES = 256_000;
const COMPRESS_EXTS = new Set([".html", ".js", ".mjs", ".css", ".svg", ".json", ".webmanifest"]);

function walk(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      walk(full, out);
    } else if (st.isFile() && !full.endsWith(".gz")) {
      out.push(full);
    }
  }
  return out;
}

const files = walk(DIST_DIR);
let totalEmbedded = 0;
let initialGzip = 0;

for (const file of files) {
  const raw = readFileSync(file);
  totalEmbedded += raw.byteLength;
  if (COMPRESS_EXTS.has(extname(file))) {
    const gz = gzipSync(raw, { level: 9 });
    writeFileSync(`${file}.gz`, gz);
    totalEmbedded += gz.byteLength;
    const rel = relative(DIST_DIR, file);
    if (rel === "index.html" || rel.startsWith("assets/index-")) {
      initialGzip += gz.byteLength;
    }
  }
}

console.log(
  `ui/dist compressed: initial_gzip=${initialGzip}B total_embedded=${totalEmbedded}B`,
);

if (initialGzip > MAX_INITIAL_GZIP_BYTES) {
  throw new Error(
    `initial gzip load ${initialGzip}B exceeds ${MAX_INITIAL_GZIP_BYTES}B budget`,
  );
}
if (totalEmbedded > MAX_EMBEDDED_BYTES) {
  throw new Error(
    `total embedded ui/dist ${totalEmbedded}B exceeds ${MAX_EMBEDDED_BYTES}B budget`,
  );
}
