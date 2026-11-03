// Cerne adapter. Upstream algorithms remain unchanged (Elastic License 2.0).
import { mkdirSync, readFileSync, writeFileSync, existsSync, unlinkSync } from "node:fs";
import { join } from "node:path";
import { createRequire } from "node:module";
import { createHash } from "node:crypto";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { CallToolRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import { z } from "zod";
import { SessionDB, resolveSessionDbPath } from "./src/session/db.js";
import { extractEvents, extractUserEvents } from "./src/session/extract.js";
import { buildResumeSnapshot } from "./src/session/snapshot.js";

// Embedded mode disables other clients' lifecycle hooks, telemetry/update checks,
// and auto-start. Cerne owns the stdio lifecycle and session identity.
process.env.CONTEXT_MODE_EMBEDDED_PLUGIN_TOOLS = "1";
const root = process.env.CONTEXT_MODE_DIR!;
const projectDir = process.env.CONTEXT_MODE_PROJECT_DIR!;
const sessionId = process.env.CLAUDE_SESSION_ID!;
if (!root || !projectDir || !sessionId) throw new Error("Missing Cerne session identity");
// Timeline auto-memory must not read another client's private config tree.
process.env.CLAUDE_CONFIG_DIR = join(root, "client-config");
const nodeVersion = process.versions.node.split(".").map(Number);
if (nodeVersion[0] < 22 || (nodeVersion[0] === 22 && nodeVersion[1] < 13)) {
  throw new Error("Economia de contexto requires Node.js >= 22.13");
}
// Bundled CJS dependencies require this in an ESM esbuild bundle.
(globalThis as any).require = createRequire(import.meta.url);
const upstream = await import("./src/server.js");
const sessionsDir = join(root, "sessions");
const contentDir = join(root, "content");
mkdirSync(sessionsDir, { recursive: true });
mkdirSync(contentDir, { recursive: true });
let db = new SessionDB({ dbPath: resolveSessionDbPath({ projectDir, sessionsDir }) });
db.ensureSession(sessionId, projectDir);
db.insertEvent(sessionId, { type: "cwd", category: "cwd", data: projectDir, priority: 1 }, "Cerne", { projectDir });
const text = (value: string) => ({ content: [{ type: "text" as const, text: value }] });
async function index(content: string, source: string) {
  const tool = upstream.REGISTERED_CTX_TOOLS.find(tool => tool.name === "ctx_index")!;
  const result = await upstream.withProjectDirOverride({ projectDir, sessionId }, async () =>
    await tool.handler({ content, source })) as any;
  if (result?.isError) throw new Error(JSON.stringify(result));
  return result;
}

async function capture(message: any, ordinal: string) {
  const name = message.name ?? "";
  const args = message.arguments ?? {};
  // Map native Cerne tool names to upstream extraction vocabulary.
  const names: Record<string, string> = {
    read_file: "Read", write_file: "Write", edit_file: "Edit", ast_edit: "Edit",
    run_command: "Bash", task: "Task", load_skill: "Skill", search_files: "Grep",
    list_files: "Glob", update_long_horizon_projeto: "TodoWrite",
  };
  const events = message.role === "user"
    ? [...extractUserEvents(message.content), {
        type: "user_prompt", category: "user-prompt", data: message.content, priority: 1,
      }]
    : message.role === "tool"
      ? extractEvents({ tool_name: names[name] ?? name,
          tool_input: { ...args, file_path: args.file_path ?? args.path },
          tool_response: message.content, tool_output: { isError: !!message.is_error } })
      : [];
  for (const event of events) db.insertEvent(sessionId, event, "Cerne", { projectDir });
  // Lossless recall safety net, including Portuguese instructions and assistant
  // decisions the English upstream extractors may not classify.
  if (message.content?.trim()) await index(
    `# ${message.role} ${name}\n${JSON.stringify(args)}\n${message.content}`,
    `cerne-history-${ordinal}`);
}

upstream.server.registerTool("cerne_capture", {
  description: "Internal Cerne event capture (not exposed to the model).",
  inputSchema: { message: z.any(), ordinal: z.string() },
}, async ({ message, ordinal }) => {
  await capture(message, ordinal);
  return text("saved");
});

upstream.server.registerTool("cerne_sync", {
  description: "Internal lossless transcript import, once on enable/resume.",
  inputSchema: { path: z.string() },
}, async ({ path }) => {
  const messages = JSON.parse(readFileSync(path, "utf8"));
  const marker = join(root, "imported.json");
  const imported: string[] = existsSync(marker) ? JSON.parse(readFileSync(marker, "utf8")) : [];
  const seen = new Set(imported);
    const calls = new Map<string, any>();
    for (const [i, message] of messages.entries()) {
      for (const call of message.tool_calls ?? []) {
        try { calls.set(call.id, JSON.parse(call.function.arguments)); } catch { /* retain text */ }
      }
      // display_content holds the full original result after output reduction.
      const content = message.role === "tool" ? (message.display_content ?? message.content) : message.content;
      const hash = createHash("sha256").update(JSON.stringify([message.role, message.tool_call_id, content])).digest("hex");
      if (!seen.has(hash) && message.role !== "system") {
        await capture({ ...message, content, arguments: calls.get(message.tool_call_id) ?? message.arguments }, `import-${hash}`);
        seen.add(hash);
      }
    }
    writeFileSync(marker, JSON.stringify([...seen]));
  return text("saved");
});

upstream.server.registerTool("cerne_resume", {
  description: "Internal pre-compaction snapshot with on-demand FTS5 recall.",
  inputSchema: { search_tool: z.string() },
}, async ({ search_tool }) => {
  const events = db.getEvents(sessionId, { limit: 10000 });
  // Use the upstream session-events source convention referenced by snapshots.
  if (events.length) await index(events.map(e => `# ${e.category}: ${e.type}\n${e.data}`).join("\n\n"), "session-events");
  const snapshot = buildResumeSnapshot(events, { searchTool: search_tool });
  db.upsertResume(sessionId, snapshot, events.length);
  return text(snapshot);
});

upstream.server.registerTool("cerne_archive", {
  description: "Internal large output archive with a small preview and search reference.",
  inputSchema: { content: z.string(), source: z.string() },
}, async ({ content, source }) => {
  // Original indexing also updates the upstream savings counters.
  return await index(content, source);
});

// Release the adapter's additional handles before the ORIGINAL purge handler
// removes SQLite files (required on Windows). Resume pointers cannot survive
// deletion of their referenced data. The transcript remains owned by Cerne.
// The pinned SDK handler-map bridge is also used by upstream's schema wrapper.
const low = upstream.server.server as any;
const callHandler = low._requestHandlers.get("tools/call");
low.setRequestHandler(CallToolRequestSchema, async (request: any, extra: any) => {
  if (request.params.name !== "ctx_purge") return callHandler(request, extra);
  const args = request.params.arguments ?? {};
  if (args.sessionId && args.sessionId !== sessionId) {
    return { ...text("Cerne: a limpeza atua somente nesta conversa."), isError: true };
  }
  // Project-wide upstream purge also scans ~/.context-mode legacy stores.
  // A Cerne process owns exactly ONE session; scoped purge gives it the same
  // local wipe while never touching another client's legacy content cache.
  const scopedRequest = { ...request, params: { ...request.params,
    arguments: { ...args, scope: "session", sessionId } } };
  db.close();
  let response: any;
  try { response = await callHandler(scopedRequest, extra); }
  finally {
    db = new SessionDB({ dbPath: resolveSessionDbPath({ projectDir, sessionsDir }) });
    db.ensureSession(sessionId, projectDir);
  }
  if (request.params.arguments?.confirm === true && !response?.isError &&
      (!request.params.arguments.sessionId || request.params.arguments.sessionId === sessionId)) {
    for (const file of ["imported.json", "view.json"]) {
      const path = join(root, file);
      if (existsSync(path)) unlinkSync(path);
    }
    db.insertEvent(sessionId, { type: "cwd", category: "cwd", data: projectDir, priority: 1 }, "Cerne", { projectDir });
  }
  return response;
});
await upstream.server.connect(new StdioServerTransport());
