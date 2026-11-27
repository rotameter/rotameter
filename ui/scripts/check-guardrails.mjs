import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { extname, join, relative } from "node:path";

const UI_ROOT = new URL("..", import.meta.url).pathname;

const CODE_RULES = [
  {
    name: "no-hex-color",
    pattern: /#[0-9a-fA-F]{3}(?:[0-9a-fA-F]{3}(?:[0-9a-fA-F]{2})?)?\b/,
    message: "Hardcoded hex colors are forbidden; use CSS tokens from tokens.css",
  },
  {
    name: "no-rgb-hsl-literal",
    pattern: /\b(?:rgba?|hsla?)\(/,
    message: "Hardcoded rgb()/hsl() colors are forbidden; use CSS tokens from tokens.css",
  },
  {
    name: "no-blurred-shadow",
    pattern: /\bshadow-(?:sm|md|lg|xl|2xl|inner)\b|\bboxShadow\b|box-shadow/,
    message: "Blurred drop-shadows violate flat hairline surface tokens",
  },
  {
    name: "no-ambiguous-text-var",
    pattern: /text-\[var\(--(?:text|density)-/,
    message: "Use text-[length:var(--...)] so Tailwind v4 emits font-size instead of color",
  },
];

const PLUGIN_ONLY_RULES = [
  {
    name: "no-window-location-in-plugin",
    pattern: /window\.location/,
    message: "Plugins must use useDashboardQuery()/updateQuery(), never window.location",
  },
  {
    name: "no-history-mutation-in-plugin",
    pattern: /history\.(?:push|replace)State/,
    message: "Plugins must use updateQuery(), never history.pushState/replaceState",
  },
  {
    name: "no-hardcoded-virtual-row-height",
    pattern: /estimateSize:\s*\(\)\s*=>\s*\d/,
    message: "Virtualizers must read --density-table-row-h dynamically, not hardcode px",
  },
];

const BANNED_DEP_RE =
  /^(echarts|echarts-for-react|three|regl|deck\.gl|@deck\.gl\/.*|pixi\.js|@luma\.gl\/.*)$/;

function walkSourceFiles(dir, out = []) {
  if (!existsSync(dir)) return out;
  for (const entry of readdirSync(dir)) {
    if (entry === "node_modules" || entry === "__tests__" || entry === "dist") continue;
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) {
      walkSourceFiles(full, out);
    } else if (st.isFile() && (extname(full) === ".ts" || extname(full) === ".tsx")) {
      out.push(full);
    }
  }
  return out;
}

const scanRoots = [join(UI_ROOT, "apps/web/src")];
const packagesDir = join(UI_ROOT, "packages");
if (existsSync(packagesDir)) {
  for (const pkg of readdirSync(packagesDir)) {
    const srcDir = join(packagesDir, pkg, "src");
    if (existsSync(srcDir)) scanRoots.push(srcDir);
  }
}

const violations = [];
let scannedFiles = 0;

for (const root of scanRoots) {
  const relRoot = relative(UI_ROOT, root);
  const isPlugin = relRoot.startsWith("packages/plugin-") && !relRoot.startsWith("packages/plugin-sdk");
  const rules = isPlugin ? [...CODE_RULES, ...PLUGIN_ONLY_RULES] : CODE_RULES;
  for (const file of walkSourceFiles(root)) {
    scannedFiles += 1;
    const relFile = relative(UI_ROOT, file);
    const lines = readFileSync(file, "utf8").split("\n");
    for (let i = 0; i < lines.length; i += 1) {
      const line = lines[i] ?? "";
      for (const rule of rules) {
        if (rule.pattern.test(line)) {
          violations.push(`${relFile}:${i + 1}: [${rule.name}] ${rule.message}`);
        }
      }
    }
  }
}

for (const group of ["apps", "packages"]) {
  const base = join(UI_ROOT, group);
  if (!existsSync(base)) continue;
  for (const entry of readdirSync(base)) {
    const pkgPath = join(base, entry, "package.json");
    if (!existsSync(pkgPath)) continue;
    const parsed = JSON.parse(readFileSync(pkgPath, "utf8"));
    for (const field of ["dependencies", "devDependencies", "peerDependencies"]) {
      const deps = parsed[field] ?? {};
      for (const name of Object.keys(deps)) {
        if (BANNED_DEP_RE.test(name)) {
          violations.push(
            `${relative(UI_ROOT, pkgPath)}: [banned-dependency] Disallowed dependency "${name}" in ${field}`,
          );
        }
      }
    }
  }
}

if (violations.length > 0) {
  console.error(violations.join("\n"));
  process.exit(1);
}

console.log(`guardrails: OK (${scannedFiles} files)`);
