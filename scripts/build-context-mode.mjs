// Build the pinned, vendored upstream + Cerne adapter. No native addon ships.
import { createRequire } from "node:module";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";
import { mkdirSync, cpSync, writeFileSync } from "node:fs";
const repo = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const vendor = resolve(repo, "src-tauri/resources/context-mode");
const dependencies = resolve(process.argv[2] ?? vendor);
const require = createRequire(resolve(dependencies, "package.json"));
const { build } = require("esbuild");
const runtime = resolve(vendor, "runtime");
mkdirSync(runtime, { recursive: true });
await build({ entryPoints: [resolve(vendor, "cerne-entry.ts")],
  outfile: resolve(runtime, "cerne.bundle.mjs"), bundle: true, platform: "node",
  target: "node22", format: "esm", minify: true,
  external: ["better-sqlite3"], nodePaths: [resolve(dependencies, "node_modules")],
  banner: { js: 'import { createRequire as __cerneRequire } from "node:module"; const require = __cerneRequire(import.meta.url);' },
});
// Upstream dynamically requires these portable JS dependencies at fetch time.
for (const name of ["turndown", "turndown-plugin-gfm", "@mixmark-io/domino"]) {
  cpSync(resolve(dependencies, "node_modules", name), resolve(runtime, "node_modules", name), { recursive: true });
}
cpSync(resolve(vendor, "LICENSE"), resolve(runtime, "LICENSE"));
writeFileSync(resolve(runtime, "package.json"), JSON.stringify({
  name: "context-mode-cerne-runtime", version: "1.0.169", type: "module", license: "Elastic-2.0",
}, null, 2));
