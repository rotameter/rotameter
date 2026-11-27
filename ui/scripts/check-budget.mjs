import { existsSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const DIST_DIR = new URL("../dist", import.meta.url).pathname;
const MAX_EMBEDDED_BYTES = 1_572_864;
const MAX_INITIAL_GZIP_BYTES = 256_000;

if (!existsSync(DIST_DIR)) {
  console.error("Missing ui/dist — run `cd ui && bun run build` first.");
  process.exit(1);
}

function walk(dir, out = []) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      walk(full, out);
    } else if (st.isFile()) {
      out.push(full);
    }
  }
  return out;
}

const files = walk(DIST_DIR);
let totalEmbedded = 0;
let initialGzip = 0;

for (const file of files) {
  const size = statSync(file).size;
  totalEmbedded += size;
  const rel = relative(DIST_DIR, file);
  if (rel === "index.html.gz" || (rel.startsWith("assets/index-") && rel.endsWith(".gz"))) {
    initialGzip += size;
  }
}

if (initialGzip === 0) {
  console.error("Missing precompressed index*.gz artifacts in ui/dist — run `cd ui && bun run build`.");
  process.exit(1);
}

console.log(
  `ui budget: initial_gzip=${initialGzip}B/${MAX_INITIAL_GZIP_BYTES}B total_embedded=${totalEmbedded}B/${MAX_EMBEDDED_BYTES}B`,
);

if (initialGzip > MAX_INITIAL_GZIP_BYTES) {
  console.error(`FAIL: initial gzip load ${initialGzip}B exceeds ${MAX_INITIAL_GZIP_BYTES}B`);
  process.exit(1);
}

if (totalEmbedded > MAX_EMBEDDED_BYTES) {
  console.error(`FAIL: total embedded ui/dist ${totalEmbedded}B exceeds ${MAX_EMBEDDED_BYTES}B`);
  process.exit(1);
}
