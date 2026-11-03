// Real stdio MCP round trips against the packaged runtime, no SDK required.
import assert from "node:assert/strict";
import { test } from "node:test";
import { spawn } from "node:child_process";
import { createInterface } from "node:readline";
import { mkdtempSync, writeFileSync, mkdirSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { resolve, join, dirname, delimiter } from "node:path";
import { fileURLToPath } from "node:url";
const runtime = fileURLToPath(new URL("../src-tauri/resources/context-mode/runtime/cerne.bundle.mjs", import.meta.url));
const nodePlatform = process.platform === "win32" ? "win" : process.platform;
const bundledNode = join(dirname(runtime), "node", `${nodePlatform}-${process.arch}`, process.platform === "win32" ? "node.exe" : "node");
const fixture = mkdtempSync(join(tmpdir(), "cerne-context-economy-"));
const project = join(fixture, "project with spaces");
mkdirSync(project);
writeFileSync(join(project, "huge.log"), "INFO all good\n".repeat(2000) + "ERROR regressao_alfa_731\n");

async function client(storage, session = "session-a", inheritedPath = globalThis.process.env.PATH) {
  const process = spawn(bundledNode, [runtime], { cwd: project, windowsHide: true,
    env: { ...globalThis.process.env, CONTEXT_MODE_DIR: resolve(storage),
      PATH: dirname(bundledNode) + delimiter + (inheritedPath || ""), NODE_OPTIONS: "", NODE_PATH: "",
      CONTEXT_MODE_PROJECT_DIR: project, CLAUDE_PROJECT_DIR: project, CLAUDE_SESSION_ID: session } });
  let sequence = 0;
  const pending = new Map();
  let stderr = "";
  process.stderr.on("data", data => stderr += data);
  createInterface({ input: process.stdout }).on("line", line => {
    try {
      const message = JSON.parse(line);
      const response = pending.get(message.id);
      if (!response) return;
      pending.delete(message.id);
      clearTimeout(response.timer);
      message.error ? response.reject(new Error(JSON.stringify(message.error))) : response.resolve(message.result);
    } catch (error) { for (const response of pending.values()) response.reject(error); }
  });
  process.on("exit", code => {
    for (const response of pending.values()) { clearTimeout(response.timer); response.reject(new Error(`Runtime exited ${code}: ${stderr}`)); }
    pending.clear();
  });
  const request = (method, params) => new Promise((resolve, reject) => {
    const id = ++sequence;
    const timer = setTimeout(() => { pending.delete(id); reject(new Error(`Timeout ${method} ${params?.name ?? ''}: ${stderr}`)); }, 30000);
    pending.set(id, { resolve, reject, timer });
    process.stdin.write(JSON.stringify({ jsonrpc: "2.0", id, method, params }) + "\n");
  });
  await request("initialize", { protocolVersion: "2024-11-05", capabilities: {}, clientInfo: { name: "cerne", version: "0.1.5" } });
  process.stdin.write(JSON.stringify({ jsonrpc: "2.0", method: "notifications/initialized" }) + "\n");
  return {
    request,
    async tool(name, args) {
      const response = await request("tools/call", { name, arguments: args });
      assert.ok(!response.isError, `${name}: ${JSON.stringify(response)}`);
      return response.content.filter(item => item.type === "text").map(item => item.text).join("\n");
    },
    async close() {
      if (process.exitCode !== null) return;
      await new Promise(resolve => { process.once("exit", resolve); process.stdin.end(); });
    },
  };
}

test("private runtime works without a system Node installation", { timeout: 60000 }, async () => {
  const osPath = process.platform === "win32" ? join(process.env.SystemRoot, "System32") : "/usr/bin:/bin";
  let c = await client(join(fixture, "no-system-node"), "standalone", osPath);
  try {
    assert.match(await c.tool("ctx_execute", { language: "javascript", code: "console.log('internocernea123', 6 * 7)" }), /internocernea123.*42/);
    await c.tool("ctx_index", { content: "Persistência autonomacernea512 funciona com SQLite privado.", source: "standalone" });
    assert.match(await c.tool("ctx_search", { queries: ["autonomacernea512"], source: "standalone" }), /SQLite/);
    await c.close();
    c = await client(join(fixture, "no-system-node"), "standalone", osPath);
    assert.match(await c.tool("ctx_search", { queries: ["autonomacernea512"], source: "standalone" }), /SQLite/);
  } finally { await c.close(); }
});

test("original tools reduce output, index, recall and persist isolated session memory", { timeout: 120000 }, async () => {
  const storage = join(fixture, "storage-a");
  let c = await client(storage);
  try {
    const listed = (await c.request("tools/list", {})).tools.map(tool => tool.name);
    for (const name of ["ctx_execute", "ctx_execute_file", "ctx_batch_execute", "ctx_index", "ctx_search", "ctx_fetch_and_index", "ctx_stats", "ctx_doctor", "ctx_purge", "ctx_insight"]) {
      assert.ok(listed.includes(name), `Missing original tool ${name}`);
    }
    const executed = await c.tool("ctx_execute", { language: "javascript", code: "console.log('resultado=' + (2 + 3));" });
    assert.match(executed, /resultado=5/);
    const file = await c.tool("ctx_execute_file", { path: join(project, "huge.log"), language: "javascript",
      code: "console.log(FILE_CONTENT.split('\\n').filter(l => l.startsWith('ERROR')).join('\\n'));" });
    assert.match(file, /regressao_alfa_731/);
    assert.ok(file.length < 1500, "Full log leaked into context");
    const batch = await c.tool("ctx_batch_execute", { commands: [{ label: "git", command: "git --version" }], queries: ["git version"] });
    assert.match(batch, /git version/i);
    await c.tool("ctx_index", { content: "# Projeto\nO compilador precisa preservar Unicode: ação, coração, 中文.\n# Decisão\nChave rara decisaocernea948: usar SQLite local.", source: "unicode-spec" });
    assert.match(await c.tool("ctx_search", { queries: ["decisaocernea948"], source: "unicode-spec" }), /SQLite/);
    const huge = "# Logs\n" + "INFO teste repetido\n".repeat(4000) + "\n# Incidente\nFalha raracerne472: encoding ISO-8859-1.\n";
    await c.tool("cerne_archive", { content: huge, source: "tool-output-a" });
    assert.match(await c.tool("ctx_search", { queries: ["raracerne472"], source: "tool-output-a" }), /ISO-8859-1/);
    const transcript = join(fixture, "chat_log.json");
    const messages = [{ role: "user", content: "Não apague os arquivos. Retome a tarefa guardadacerne193." },
      { role: "tool", name: "edit_file", content: "diff aplicado", arguments: { path: "main.rs" } }];
    writeFileSync(transcript, JSON.stringify(messages));
    await c.tool("cerne_sync", { path: transcript });
    await c.tool("cerne_sync", { path: transcript }); // idempotent import
    await c.tool("cerne_capture", { ordinal: "tool-1", message: { role: "tool", name: "run_command",
      arguments: { command: "git status" }, content: "On branch main", is_error: false } });
    const snapshot = await c.tool("cerne_resume", { search_tool: "mcp__cerne__ctx_search" });
    assert.match(snapshot, /guardadacerne193/);
    assert.match(snapshot, /mcp__cerne__ctx_search/);
    assert.ok(snapshot.length < huge.length / 10);
    const forbidden = await c.request("tools/call", { name: "ctx_fetch_and_index", arguments: { url: "http://127.0.0.1:12345/private" } });
    assert.ok(forbidden.isError || JSON.stringify(forbidden).match(/blocked|private|SSRF/i), "Fetch must retain upstream SSRF protection");
    await c.close();
    c = await client(storage);
    assert.match(await c.tool("ctx_search", { queries: ["raracerne472"], source: "tool-output-a" }), /ISO-8859-1/);
    assert.match(await c.tool("cerne_resume", { search_tool: "mcp__cerne__ctx_search" }), /guardadacerne193/);
    // Import messages written while the mode was off, despite an existing marker.
    messages.push({ role: "user", content: "intervalocernea887 preservar esta mudança" });
    writeFileSync(transcript, JSON.stringify(messages));
    await c.tool("cerne_sync", { path: transcript });
    assert.match(await c.tool("ctx_search", { queries: ["intervalocernea887"] }), /preservar/);
    const stats = await c.tool("ctx_stats", {});
    assert.match(stats, /indexed|sandbox|saving|context/i);
    await c.tool("ctx_purge", { confirm: true, scope: "project" });
    // Purge cannot leave a view pointing at deleted content; import the actual
    // transcript again and verify the live adapter reopened its DB handles.
    await c.tool("cerne_sync", { path: transcript });
    assert.match(await c.tool("ctx_search", { queries: ["intervalocernea887"] }), /preservar/);
  } finally { await c.close(); }
  const isolated = await client(join(fixture, "storage-b"), "session-b");
  try {
    const search = await isolated.request("tools/call", { name: "ctx_search", arguments: { queries: ["raracerne472"] } });
    assert.doesNotMatch(JSON.stringify(search), /ISO-8859-1/);
  } finally { await isolated.close(); }
  console.log(`Verified real SQLite files at ${fixture}; log input ${readFileSync(join(project, 'huge.log')).length} bytes.`);
});
