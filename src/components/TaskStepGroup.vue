<script setup lang="ts">
import { ref, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import { api, type TaskItem, type AgentExecution } from "../api";
import { friendlyStepLabel, toolNameFromLabel, formatElapsed } from "../taskLabels";
import { splitDiffDetail, diffLines } from "../diffUtils";

const { t } = useI18n();

defineProps<{ tasks: TaskItem[] }>();

// Fase 3/T14 (achado testando ao vivo, 2026-08-16): os passos internos de
// `task`/`verify_completion`/`run_pipeline` (o que o sub-agente/QA/Analista
// fez por dentro) só existiam como evento efêmero de UI — sumiam assim que
// o turno terminava. Agora ficam gravados em `AgentExecution.steps`
// (backend), buscados sob demanda aqui quando o usuário expande o item —
// mesmo espírito "auditoria disponível, mas escondida até pedir" do resto
// do componente (IN/OUT, diff).
const allExecutions = ref<AgentExecution[]>([]);
const loadingExecutions = ref<Set<string>>(new Set());

async function ensureExecutionLoaded(executionId: string) {
  if (loadingExecutions.value.has(executionId)) return;
  loadingExecutions.value.add(executionId);
  try {
    allExecutions.value = await api.listAgentExecutions();
  } finally {
    loadingExecutions.value.delete(executionId);
  }
}

function executionById(id: string): AgentExecution | undefined {
  return allExecutions.value.find((e) => e.id === id);
}

// Pipeline não roda ferramenta nenhuma sozinho (`steps` dele fica sempre
// vazio) — quem tem passos de verdade são as execuções filhas (dev/qa/
// analista por round, ligadas via `parent_id`).
function childExecutions(id: string): AgentExecution[] {
  return allExecutions.value
    .filter((e) => e.parent_id === id)
    .sort((a, b) => a.started_at_ms - b.started_at_ms);
}

function executionStatusLabel(status: AgentExecution["status"]): string {
  return t(`agentExecutions.status.${status}`);
}

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

function toggle(t: TaskItem) {
  const wasExpanded = expanded.value.has(t.id);
  toggleSet(expanded, t.id);
  if (!wasExpanded && t.execution_id) {
    ensureExecutionLoaded(t.execution_id);
  }
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

// Enquanto a tool call ainda está rodando, `detail`/`command`/etc chegam aos
// poucos via eventos (`agent:tool_call` dá o "IN" na hora, `agent:tool_result`
// dá o "OUT" quando termina) — sem precisar recarregar a sessão inteira pra
// ver o resultado. Esse placeholder cobre o intervalo entre os dois.
function outText(task: TaskItem, isExpanded: boolean): string {
  if (task.detail == null && task.status === "running") {
    return t("taskStep.processing");
  }
  return previewText(task.detail, isExpanded);
}

// So o backend preenche `command` pra chamadas de `run_command` (ver
// `extract_command_text` em agent/mod.rs) — checar só esse campo, em vez de
// tentar reconhecer o nome da ferramenta a partir do label, também funciona
// pra passos de sub-agente/verificador (label vem com prefixo tipo
// "↳ sub-agente (...): run_command").
function isCommandTool(t: TaskItem): boolean {
  return !!t.command;
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

// splitDiffDetail/diffLines: ver src/diffUtils.ts (compartilhado com
// RepoDiffViewer.vue, Fase D1 do roteiro de Agentes/Skills).
</script>

<template>
  <div class="step-group">
    <template v-for="t in tasks" :key="t.id">
      <div
        class="step-row"
        :class="{ clickable: !isCommandTool(t) }"
        @click="!isCommandTool(t) && toggle(t)"
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
                ? $t("taskStep.showLess")
                : $t("taskStep.showMoreLines", { count: linesInfo(t.command).lines.length - PREVIEW_LINES })
            }}
          </button>
        </div>
        <div class="cmd-block">
          <div class="cmd-block-label">OUT</div>
          <pre class="cmd-box" :class="{ 'cmd-box-pending': t.detail == null && t.status === 'running' }">{{ outText(t, expandedOut.has(t.id)) }}</pre>
          <button v-if="linesInfo(t.detail).hasMore" class="cmd-more" @click.stop="toggleOut(t.id)">
            {{
              expandedOut.has(t.id)
                ? $t("taskStep.showLess")
                : $t("taskStep.showMoreLines", { count: linesInfo(t.detail).lines.length - PREVIEW_LINES })
            }}
          </button>
        </div>
      </div>
      <div v-else-if="isWriteTool(t) && expanded.has(t.id)" class="step-detail diff-detail">
        <template v-if="t.detail == null && t.status === 'running'">
          <div class="step-detail-label">{{ $t("taskStep.processing") }}</div>
        </template>
        <template v-else>
          <div class="step-detail-label">{{ splitDiffDetail(t.detail).note }}</div>
          <div class="diff-box">
            <div v-for="(line, idx) in diffLines(t.detail)" :key="idx" class="diff-line" :class="`diff-${line.kind}`">{{ line.text || " " }}</div>
          </div>
        </template>
      </div>
      <div v-else-if="expanded.has(t.id)" class="step-detail">
        <div class="step-detail-label">{{ friendlyStepLabel(t.label) }}</div>
        <div v-if="t.detail" class="step-detail-body">{{ t.detail }}</div>
        <div v-else-if="t.status === 'running'" class="step-detail-body step-detail-pending">{{ $t("taskStep.processing") }}</div>
        <div v-if="t.execution_id" class="nested-execution">
          <p v-if="loadingExecutions.has(t.execution_id)" class="hint">{{ $t("taskStep.processing") }}</p>
          <template v-else-if="executionById(t.execution_id)">
            <!-- Pipeline: etapas filhas (dev/qa/analista por round), cada -->
            <!-- uma com seus próprios passos aninhados. -->
            <div v-for="child in childExecutions(t.execution_id)" :key="child.id" class="nested-exec-group">
              <div class="nested-exec-header">
                <span class="msi status" :class="child.status">{{ statusIcon[child.status] ?? "schedule" }}</span>
                {{ child.name }} — {{ executionStatusLabel(child.status) }}
              </div>
              <TaskStepGroup :tasks="child.steps" />
            </div>
            <!-- task/verify_completion direto: passos da própria execução. -->
            <TaskStepGroup
              v-if="executionById(t.execution_id)!.steps.length > 0"
              :tasks="executionById(t.execution_id)!.steps"
            />
          </template>
        </div>
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
  font-family: var(--cerne-mono);
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
  font-family: var(--cerne-mono);
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
  font-family: var(--cerne-mono);
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
  font-family: var(--cerne-mono);
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

.cmd-box-pending,
.step-detail-pending {
  font-style: italic;
  color: #a1a1aa;
}

.nested-execution {
  margin-top: 6px;
}

.nested-execution .hint {
  font-size: 11px;
  font-style: italic;
  color: #a1a1aa;
}

.nested-exec-group {
  margin-bottom: 4px;
  padding: 4px 0 4px 8px;
  border-left: 2px solid #e4e4e7;
}

.nested-exec-group:last-child {
  margin-bottom: 0;
}

.nested-exec-header {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-weight: 600;
  color: #52525b;
}

.nested-exec-header .status {
  font-size: 13px;
  color: #a1a1aa;
}

.nested-exec-header .status.done {
  color: #16a34a;
}

.nested-exec-header .status.failed {
  color: #dc2626;
}

.nested-exec-header .status.running {
  color: #3f3f46;
  animation: spin 1s linear infinite;
}

.diff-detail {
  background: #1e1e1e;
}

.diff-detail .step-detail-label {
  color: #9d9d9d;
}

.diff-box {
  margin-top: 6px;
  border-radius: 6px;
  overflow: hidden auto;
  max-height: 320px;
  font-family: var(--cerne-mono);
  font-size: 11.5px;
  line-height: 1.5;
}

.diff-line {
  padding: 0 8px;
  white-space: pre-wrap;
  overflow-wrap: break-word;
  color: #d4d4d4;
}

.diff-line.diff-add {
  background: rgba(46, 160, 67, 0.2);
  color: #7ee787;
}

.diff-line.diff-del {
  background: rgba(248, 81, 73, 0.2);
  color: #ff9492;
}

.diff-line.diff-hunk {
  color: #a371f7;
  background: rgba(163, 113, 247, 0.1);
}

.diff-line.diff-header {
  color: #8b949e;
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
  font-family: var(--cerne-mono);
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
