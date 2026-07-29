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

function toggle(id: string) {
  if (expanded.value.has(id)) expanded.value.delete(id);
  else expanded.value.add(id);
  expanded.value = new Set(expanded.value);
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
    <div v-for="t in tasks" :key="t.id" class="step-row" @click="toggle(t.id)">
      <span class="msi status" :class="t.status">{{ statusIcon[t.status] ?? "schedule" }}</span>
      <span class="step-label">{{ friendlyStepLabel(t.label) }}</span>
      <span v-if="t.file_path && isFileTool(t)" class="file-chip" :title="t.file_path">
        <span class="msi file-icon">{{ fileIcon(t.file_path) }}</span>
        <span class="file-name">{{ fileName(t.file_path) }}</span>
      </span>
      <span v-if="isWriteTool(t) && (t.additions || t.deletions)" class="diff-stats">
        <span v-if="t.additions" class="stat-add">+{{ t.additions }}</span>
        <span v-if="t.deletions" class="stat-del">-{{ t.deletions }}</span>
      </span>
      <span v-if="taskElapsed(t)" class="step-elapsed">{{ taskElapsed(t) }}</span>
      <span class="msi chevron" :class="{ open: expanded.has(t.id) }">chevron_right</span>
    </div>
    <template v-for="t in tasks" :key="`detail-${t.id}`">
      <div v-if="expanded.has(t.id)" class="step-detail">
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
  cursor: pointer;
  font-size: 12.5px;
  font-weight: 400;
  color: #71717a;
}

.step-row:hover {
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
  margin-left: auto;
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

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
