// Developer-only oracle. Never runs or ships in the native application.
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { mkdtempSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { tmpdir } from "node:os";
const root = mkdtempSync(join(tmpdir(), "cerne-context-reference-"));
const child = spawn(process.execPath, ["src-tauri/resources/context-mode/runtime/cerne.bundle.mjs"], {
  env: { ...process.env, CONTEXT_MODE_DIR: root, CONTEXT_MODE_PROJECT_DIR: root, CLAUDE_SESSION_ID: "reference" }, windowsHide: true,
});
let sequence = 0;
const pending = new Map();
createInterface({ input: child.stdout }).on("line", line => {
  const value = JSON.parse(line);
  const resolve = pending.get(value.id);
  if (resolve) { pending.delete(value.id); resolve(value.result); }
});
child.stderr.on("data", bytes => process.stderr.write(bytes));
const rpc = (method, params = {}) => new Promise(resolve => {
  const id = ++sequence; pending.set(id, resolve);
  child.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
});
await rpc("initialize", { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "reference", version: "1" } });
child.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
const { tools } = await rpc("tools/list");
mkdirSync("src-tauri/src/context_economy", { recursive: true });
writeFileSync("src-tauri/src/context_economy/reference-tools.json", JSON.stringify(tools.filter(t => !t.name.startsWith("cerne_") && t.name !== "ctx_upgrade"), null, 2));
console.log(tools.map(t => t.name).join(", "));
child.stdin.end();
