<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import type { TaskItem } from "../api";
import { friendlyStepLabel, toolNameFromLabel, formatElapsed } from "../taskLabels";

defineProps<{ tasks: TaskItem[] }>();

const now = ref(Date.now());
let timer: ReturnType<typeof setInterval> | null = null;

onMounted(() => {
  timer = setInterval(() => { now.value = Date.now(); }, 1000);
});
onUnmounted(() => { if (timer) clearInterval(timer); });

function taskElapsed(t: TaskItem): string {
  if (t.duration_ms != null) return formatElapsed(t.duration_ms);
  if (t.started_at_ms && t.status === "running") return formatElapsed(now.value - t.started_at_ms);
  return "";
}

const expanded = ref<Set<string>>(new Set());
const expandedIn = ref<Set<string>>(new Set());
const expandedOut = ref<Set<string>>(new Set());

function toggleSet(set: typeof expanded, id: string) {
  if (set.value.has(id)) set.value.delete(id);
  else set.value.add(id);
  set.value = new Set(set.value);
}

function toggle(id: string) {
  toggleSet(expanded, id);
}

function toggleIn(id: string) {
  toggleSet(expandedIn, id);
}

function toggleOut(id: string) {
  toggleSet(expandedOut, id);
}

// Quantas linhas mostrar antes de precisar expandir — como um preview de
// terminal (bloco "IN"/"OUT"), o resto fica escondido atrás de um botão.
const PREVIEW_LINES = 4;

function linesInfo(text: string | null | undefined) {
  const full = text ?? "";
  const lines = full.split("\n");
  return { full, lines, hasMore: lines.length > PREVIEW_LINES };
}

function previewText(text: string | null | undefined, isExpanded: boolean): string {
  const { full, lines, hasMore } = linesInfo(text);
  if (!hasMore || isExpanded) return full;
  return lines.slice(0, PREVIEW_LINES).join("\n");
}

function isCommandTool(t: TaskItem): boolean {
  return toolNameFromLabel(t.label) === "run_command" && !!t.command;
}

const statusIcon: Record<string, string> = {
  pending: "schedule",
  running: "progress_activity",
  done: "check_circle",
  failed: "error",
};

function fileName(path: string): string {
  return path.split(/[/\\]/).filter(Boolean).pop() ?? path;
}

const FILE_ICONS: Record<string, string> = {
  rs: "memory",
  ts: "javascript",
  tsx: "javascript",
  js: "javascript",
  jsx: "javascript",
  vue: "web",
  py: "terminal",
  go: "terminal",
  java: "coffee",
  md: "description",
  json: "data_object",
  yaml: "settings",
  yml: "settings",
  toml: "settings",
  html: "language",
  css: "palette",
  sql: "database",
  sh: "terminal",
  txt: "article",
};

function fileIcon(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return FILE_ICONS[ext] ?? "description";
}

function isFileTool(task: TaskItem): boolean {
  const name = toolNameFromLabel(task.label);
  return ["read_file", "write_file", "edit_file", "ast_edit", "ast_grep", "list_dir", "grep"].includes(name);
}

function isWriteTool(task: TaskItem): boolean {
  const name = toolNameFromLabel(task.label);
  return ["write_file", "edit_file", "ast_edit"].includes(name);
}
</script>

<template>
  <div class="step-group">
    <template v-for="t in tasks" :key="t.id">
      <div
        class="step-row"
        :class="{ clickable: !isCommandTool(t) }"
        @click="!isCommandTool(t) && toggle(t.id)"
      >
        <span class="msi status" :class="t.status">{{ statusIcon[t.status] ?? "schedule" }}</span>
        <span v-if="taskElapsed(t)" class="step-elapsed">({{ taskElapsed(t) }})</span>
        <span class="step-label">{{ friendlyStepLabel(t.label) }}</span>
        <span v-if="t.file_path && isFileTool(t)" class="file-chip" :title="t.file_path">
          <span class="msi file-icon">{{ fileIcon(t.file_path) }}</span>
          <span class="file-name">{{ fileName(t.file_path) }}</span>
        </span>
        <span v-if="isWriteTool(t) && (t.additions || t.deletions)" class="diff-stats">
          <span v-if="t.additions" class="stat-add">+{{ t.additions }}</span>
          <span v-if="t.deletions" class="stat-del">-{{ t.deletions }}</span>
        </span>
        <span v-if="!isCommandTool(t)" class="msi chevron" :class="{ open: expanded.has(t.id) }">chevron_right</span>
      </div>
      <!-- Comandos mostram o preview IN/OUT direto, sem precisar clicar -->
      <div v-if="isCommandTool(t)" class="cmd-inline">
        <div class="cmd-block">
          <div class="cmd-block-label">IN</div>
          <pre class="cmd-box">{{ previewText(t.command, expandedIn.has(t.id)) }}</pre>
          <button v-if="linesInfo(t.command).hasMore" class="cmd-more" @click.stop="toggleIn(t.id)">
            {{
              expandedIn.has(t.id)
                ? "mostrar menos"
                : `mostrar mais ${linesInfo(t.command).lines.length - PREVIEW_LINES} linhas`
            }}
          </button>
        </div>
        <div class="cmd-block">
          <div class="cmd-block-label">OUT</div>
          <pre class="cmd-box">{{ previewText(t.detail, expandedOut.has(t.id)) }}</pre>
          <button v-if="linesInfo(t.detail).hasMore" class="cmd-more" @click.stop="toggleOut(t.id)">
            {{
              expandedOut.has(t.id)
                ? "mostrar menos"
                : `mostrar mais ${linesInfo(t.detail).lines.length - PREVIEW_LINES} linhas`
            }}
          </button>
        </div>
      </div>
      <div v-else-if="expanded.has(t.id)" class="step-detail">
        <div class="step-detail-label">{{ t.label }}</div>
        <div v-if="t.detail" class="step-detail-body">{{ t.detail }}</div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.step-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 6px 0;
  padding: 6px 2px;
}

.step-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 3px 4px;
  border-radius: 6px;
  font-size: 12.5px;
  font-weight: 400;
  color: #71717a;
}

.step-row.clickable {
  cursor: pointer;
}

.step-row.clickable:hover {
  background: #f4f4f5;
  color: #3f3f46;
}

.step-row .status {
  font-size: 15px;
  color: #a1a1aa;
  flex-shrink: 0;
}

.step-row .status.done {
  color: #16a34a;
}

.step-row .status.failed {
  color: #dc2626;
}

.step-row .status.running {
  color: #3f3f46;
  animation: spin 1s linear infinite;
}

.step-label {
  flex-shrink: 0;
  max-width: 200px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.file-chip {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  background: #f4f4f5;
  border: 1px solid #e4e4e7;
  border-radius: 4px;
  padding: 1px 6px 1px 4px;
  font-size: 11px;
  font-weight: 500;
  font-family: ui-monospace, "Cascadia Code", "Fira Code", monospace;
  color: #3f3f46;
  max-width: 220px;
  flex-shrink: 1;
  min-width: 0;
}

.file-chip .file-icon {
  font-size: 13px;
  color: #71717a;
  flex-shrink: 0;
}

.file-chip .file-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.diff-stats {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  font-weight: 600;
  font-family: ui-monospace, monospace;
  flex-shrink: 0;
}

.stat-add {
  color: #16a34a;
}

.stat-del {
  color: #dc2626;
}

.step-elapsed {
  font-size: 11px;
  font-weight: 400;
  color: #a1a1aa;
  font-family: ui-monospace, monospace;
  flex-shrink: 0;
}

.cmd-inline {
  margin: 0 0 4px 23px;
  padding: 6px 10px;
  background: #fafafa;
  border: var(--cerne-border);
  border-radius: 8px;
}

.chevron {
  font-size: 16px;
  color: #d4d4d8;
  flex-shrink: 0;
  margin-left: auto;
  transition: transform 0.15s ease;
}

.chevron.open {
  transform: rotate(90deg);
}

.step-detail {
  margin: 0 0 2px 23px;
  padding: 6px 10px;
  background: #fafafa;
  border: var(--cerne-border);
  border-radius: 8px;
}

.step-detail-label {
  font-size: 11px;
  font-weight: 500;
  font-family: ui-monospace, monospace;
  color: #52525b;
  overflow-wrap: break-word;
}

.step-detail-body {
  margin-top: 4px;
  font-size: 11px;
  font-weight: 400;
  color: #71717a;
  white-space: pre-wrap;
  overflow-wrap: break-word;
  max-height: 240px;
  overflow-y: auto;
}

.cmd-block {
  margin-bottom: 6px;
}

.cmd-block:last-child {
  margin-bottom: 0;
}

.cmd-block-label {
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.06em;
  color: #a1a1aa;
  margin-bottom: 2px;
}

.cmd-box {
  margin: 0;
  padding: 6px 8px;
  background: #f4f4f5;
  border: 1px solid #e4e4e7;
  color: #3f3f46;
  border-radius: 6px;
  font-size: 11px;
  font-family: ui-monospace, "Cascadia Code", "Fira Code", monospace;
  white-space: pre-wrap;
  overflow-wrap: break-word;
  max-height: 200px;
  overflow-y: auto;
}

.cmd-more {
  margin-top: 3px;
  padding: 0;
  background: none;
  border: none;
  font-size: 10.5px;
  color: #6366f1;
  cursor: pointer;
}

.cmd-more:hover {
  text-decoration: underline;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
