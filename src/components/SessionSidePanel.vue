<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import {
  api,
  type AgentExecution,
  type AnythingSource,
  type AnythingTarget,
  type BackgroundJobInfo,
  type BrowserProfile,
  type GitFileChange,
  type McpServerConfig,
} from "../api";
import { useSessionStore } from "../stores/session";
import ContextGauge from "./ContextGauge.vue";
import ProcessHistoryModal from "./ProcessHistoryModal.vue";
import AnythingSetupDialog from "./AnythingSetupDialog.vue";
import AnythingFinalPrompt from "./AnythingFinalPrompt.vue";
import { useProviderStore } from "../stores/provider";

// Painel lateral da sessão atual (espelha o painel direito do Claude Desktop):
// ambiente (pastas e permissão), contexto, MCPs ativos, alterações do git,
// processos em segundo plano, agentes e sub-sessões. Só lê dados que já
// existem no app — não cria nada novo no backend.

const { t } = useI18n();
const sessionStore = useSessionStore();

const mcpServers = ref<McpServerConfig[]>([]);
const gitChanges = ref<GitFileChange[]>([]);
const gitError = ref<string | null>(null);
const jobs = ref<BackgroundJobInfo[]>([]);
const executions = ref<AgentExecution[]>([]);
const collapsed = ref<Record<string, boolean>>({});
// Modo Anything: alvos da varredura (`.anything/alvos.json`) e o resultado da última ação.
const anythingTargets = ref<AnythingTarget[]>([]);
const anythingSources = ref<AnythingSource[]>([]);
const anythingIgnored = ref(0);
const anythingMsg = ref("");
const anythingBusy = ref(false);
const anythingSetupVisible = ref(false);
const providerStore = useProviderStore();
const anythingFinalDefault = ref("");
api.anythingFinalDefault().then((t) => (anythingFinalDefault.value = t)).catch(() => {});
// O que vale quando a sessão não escreveu o próprio: o texto das Configurações, ou o padrão do Cerne.
const anythingFinalFallback = computed(() => providerStore.config?.anything?.final_prompt?.trim() || anythingFinalDefault.value);
// Linha de comando que sobe o servidor do modelo desta sessão. Só roda ao enviar uma mensagem com o servidor
// desligado; abrir ou criar a sessão não sobe nada.
const serverCommand = ref("");
const serverMsg = ref("");
watch(
  () => sessionStore.currentSession?.server_command,
  (v) => (serverCommand.value = v ?? ""),
  { immediate: true },
);
async function saveServerCommand() {
  if ((sessionStore.currentSession?.server_command ?? "") === serverCommand.value.trim()) return;
  try {
    await sessionStore.updateServerCommand(serverCommand.value);
    serverMsg.value = "";
  } catch (e) {
    serverMsg.value = String(e);
  }
}

async function saveSessionFinal(value: string) {
  try {
    await sessionStore.updateAnythingFinal(value);
  } catch (e) {
    anythingMsg.value = String(e);
  }
}
// Processo aberto no modal de historico (guardado por id, pra acompanhar a saida ao vivo).
const selectedJobId = ref<string | null>(null);
const selectedJob = computed(() => jobs.value.find((j) => j.id === selectedJobId.value) ?? null);

const session = computed(() => sessionStore.currentSession);

const mcpNames = computed(() => {
  const s = session.value;
  if (!s) return [];
  // Mesma regra do ComposerBar: `null` significa "todos os MCPs configurados".
  if (!s.enabled_mcp_servers) return mcpServers.value.map((m) => m.name);
  return s.enabled_mcp_servers;
});

const children = computed(() =>
  sessionStore.sessions.filter((s) => s.parent_session_id === sessionStore.currentId),
);
const parent = computed(() => {
  const pid = session.value?.parent_session_id;
  return pid ? sessionStore.sessions.find((s) => s.id === pid) ?? null : null;
});

const runningJobs = computed(() => jobs.value.filter((j) => j.status.kind === "running").length);
const runningAgents = computed(() => executions.value.filter((e) => e.status === "running").length);

const browserProfiles = ref<BrowserProfile[]>([]);
const newProfileLabel = ref("");
const newProfileBrowser = ref<"chrome" | "msedge">("chrome");

async function loadBrowserProfiles() {
  try {
    browserProfiles.value = await api.listBrowserProfiles();
  } catch {
    browserProfiles.value = [];
  }
}

async function chooseBrowserProfile(value: string) {
  try {
    await sessionStore.setBrowserProfile(value || null);
  } catch (e) {
    sessionStore.error = String(e);
  }
}

async function createBrowserProfile() {
  try {
    const profile = await api.createBrowserProfile(newProfileLabel.value, newProfileBrowser.value);
    browserProfiles.value.push(profile);
    newProfileLabel.value = "";
    await chooseBrowserProfile(profile.id);
  } catch (e) {
    sessionStore.error = String(e);
  }
}

async function loadMcp() {
  try {
    mcpServers.value = await api.listMcpServers();
  } catch {
    mcpServers.value = [];
  }
}

// Pasta usada para o git: a pasta de projeto ou, sem ela, a primeira pasta extra com
// permissão de edição (sessão que trabalha só numa pasta anexada também tem alterações).
const gitRoot = computed(
  () =>
    session.value?.project_root ||
    session.value?.extra_read_paths?.find((p) => p.mode === "read_write")?.path ||
    null,
);

async function refreshGit() {
  const root = gitRoot.value;
  if (!root) {
    gitChanges.value = [];
    gitError.value = null;
    return;
  }
  try {
    gitChanges.value = await api.gitRepoStatus(root);
    gitError.value = null;
  } catch {
    gitChanges.value = [];
    gitError.value = "not-repo";
  }
}

async function refreshJobsAndAgents() {
  const id = sessionStore.currentId;
  if (!id) return;
  try {
    jobs.value = (await api.listBackgroundJobs()).filter((j) => j.session_id === id);
  } catch {
    jobs.value = [];
  }
  try {
    executions.value = (await api.listAgentExecutions()).filter((e) => e.session_id === id);
  } catch {
    executions.value = [];
  }
}

// Atualiza jobs/agentes a cada 5s e o git a cada 15s (git status pode ser
// caro em repositórios grandes, então não roda no mesmo ritmo).
let tick = 0;
let poll: ReturnType<typeof setInterval> | null = null;
function refreshAll() {
  refreshJobsAndAgents();
  if (tick % 3 === 0) refreshGit();
  tick++;
}

onMounted(() => {
  loadMcp();
  loadBrowserProfiles();
  refreshAll();
  anythingRefresh();
  poll = setInterval(() => {
    refreshAll();
    anythingRefresh();
  }, 5000);
});
onUnmounted(() => {
  if (poll) clearInterval(poll);
});
watch(
  () => sessionStore.currentId,
  () => {
    tick = 0;
    refreshAll();
    anythingRefresh();
  },
);
watch(() => session.value?.anything?.enabled, anythingRefresh);

async function anythingRefresh() {
  const id = sessionStore.currentId;
  if (!id || !session.value?.anything?.enabled) {
    anythingTargets.value = [];
    anythingSources.value = [];
    return;
  }
  try {
    anythingTargets.value = await api.anythingTargets(id);
    anythingSources.value = await api.anythingSources(id);
  } catch {
    anythingTargets.value = [];
  }
}

// Adiciona pastas ou arquivos à lista de fontes e já varre, para os alvos aparecerem.
async function anythingAdd(directory: boolean) {
  const id = sessionStore.currentId;
  if (!id) return;
  const escolhido = await open({ directory, multiple: true });
  if (!escolhido) return;
  const caminhos = Array.isArray(escolhido) ? escolhido : [escolhido];
  anythingBusy.value = true;
  anythingMsg.value = "";
  try {
    for (const c of caminhos) {
      anythingSources.value = await api.anythingAddSource(id, c);
    }
    const r = await api.anythingScan(id);
    anythingTargets.value = r.alvos;
    anythingIgnored.value = r.ignorados;
  } catch (e) {
    anythingMsg.value = String(e);
  } finally {
    anythingBusy.value = false;
  }
}

async function anythingRemove(caminho: string) {
  const id = sessionStore.currentId;
  if (!id) return;
  anythingMsg.value = "";
  try {
    anythingSources.value = await api.anythingRemoveSource(id, caminho);
    anythingTargets.value = await api.anythingTargets(id);
  } catch (e) {
    anythingMsg.value = String(e);
  }
}

async function anythingScan() {
  const id = sessionStore.currentId;
  if (!id) return;
  anythingBusy.value = true;
  anythingMsg.value = "";
  try {
    const r = await api.anythingScan(id);
    anythingTargets.value = r.alvos;
    anythingIgnored.value = r.ignorados;
  } catch (e) {
    anythingMsg.value = String(e);
  } finally {
    anythingBusy.value = false;
  }
}

async function anythingPlan() {
  const id = sessionStore.currentId;
  if (!id) return;
  anythingBusy.value = true;
  anythingMsg.value = "";
  try {
    const r = await api.anythingPlanBatch(id, "completo", 150);
    anythingMsg.value = t("sidePanel.anything.planned", { n: r.length });
    await anythingRefresh();
  } catch (e) {
    anythingMsg.value = String(e);
  } finally {
    anythingBusy.value = false;
  }
}

function anythingStatusLabel(status: string) {
  return t("sidePanel.anything.status." + status);
}

function toggle(key: string) {
  collapsed.value[key] = !collapsed.value[key];
}

function openSession(id: string) {
  sessionStore.selectSession(id);
}

function statusLabel(status: string): string {
  return t(`sidePanel.status.${status}`, status);
}
</script>

<template>
  <aside class="side-panel">
    <!-- Ambiente -->
    <section class="sp-section">
      <button class="sp-head" @click="toggle('env')">
        <span class="msi chevron">{{ collapsed.env ? "chevron_right" : "expand_more" }}</span>
        <span class="sp-title">{{ $t("sidePanel.sec.env") }}</span>
      </button>
      <div v-if="!collapsed.env" class="sp-body">
        <div v-if="session?.project_root" class="sp-row">
          <span class="msi row-icon">folder</span>
          <div class="row-main">
            <span class="row-text" :title="session.project_root">{{ session.project_root }}</span>
            <span class="row-badge write">{{ $t("sidePanel.env.readWrite") }}</span>
          </div>
        </div>
        <p v-else class="sp-empty">{{ $t("sidePanel.env.none") }}</p>
        <div v-for="p in session?.extra_read_paths ?? []" :key="p.path" class="sp-row">
          <span class="msi row-icon">folder_open</span>
          <div class="row-main">
            <span class="row-text" :title="p.path">{{ p.path }}</span>
            <span class="row-badge" :class="p.mode === 'read_write' ? 'write' : 'read'">
              {{ p.mode === "read_write" ? $t("sidePanel.env.readWrite") : $t("sidePanel.env.readOnly") }}
            </span>
          </div>
        </div>
      </div>
    </section>

    <!-- Contexto -->
    <section class="sp-section">
      <button class="sp-head" @click="toggle('context')">
        <span class="msi chevron">{{ collapsed.context ? "chevron_right" : "expand_more" }}</span>
        <span class="sp-title">{{ $t("sidePanel.sec.context") }}</span>
      </button>
      <div v-if="!collapsed.context" class="sp-body">
        <ContextGauge />
      </div>
    </section>

    <!-- MCPs -->
    <section class="sp-section">
      <button class="sp-head" @click="toggle('mcp')">
        <span class="msi chevron">{{ collapsed.mcp ? "chevron_right" : "expand_more" }}</span>
        <span class="sp-title">{{ $t("sidePanel.sec.mcp") }}</span>
        <span class="sp-count">{{ mcpNames.length }}</span>
      </button>
      <div v-if="!collapsed.mcp" class="sp-body">
        <div v-for="name in mcpNames" :key="name" class="sp-row">
          <span class="status-dot ok"></span>
          <span class="row-text">{{ name }}</span>
        </div>
        <p v-if="!mcpNames.length" class="sp-empty">{{ $t("sidePanel.mcp.none") }}</p>
      </div>
    </section>

    <!-- Navegador -->
    <section class="sp-section">
      <button class="sp-head" @click="toggle('browser')">
        <span class="msi chevron">{{ collapsed.browser ? "chevron_right" : "expand_more" }}</span>
        <span class="sp-title">{{ $t("sidePanel.sec.browser") }}</span>
      </button>
      <div v-if="!collapsed.browser" class="sp-body">
        <select
          class="sp-select"
          :value="session?.browser_profile ?? ''"
          :aria-label="$t('sidePanel.sec.browser')"
          @change="chooseBrowserProfile(($event.target as HTMLSelectElement).value)"
        >
          <option value="">{{ $t("sidePanel.browser.default") }}</option>
          <option v-for="p in browserProfiles" :key="p.id" :value="p.id">{{ p.label }} ({{ p.browser === "msedge" ? "Edge" : "Chrome" }})</option>
        </select>
        <p class="sp-empty">{{ $t("sidePanel.browser.hint") }}</p>
        <div class="sp-create">
          <input v-model="newProfileLabel" class="sp-input" :placeholder="$t('sidePanel.browser.newPlaceholder')" />
          <select v-model="newProfileBrowser" class="sp-select">
            <option value="chrome">Chrome</option>
            <option value="msedge">Edge</option>
          </select>
          <button class="sp-btn" :disabled="!newProfileLabel.trim()" @click="createBrowserProfile">
            {{ $t("sidePanel.browser.create") }}
          </button>
        </div>
      </div>
    </section>

    <!-- Servidor do modelo -->
    <section class="sp-section">
      <button class="sp-head" @click="toggle('server')">
        <span class="msi chevron">{{ collapsed.server ? "chevron_right" : "expand_more" }}</span>
        <span class="sp-title">{{ $t("sidePanel.sec.server") }}</span>
      </button>
      <div v-if="!collapsed.server" class="sp-body">
        <input
          v-model="serverCommand"
          class="sp-input"
          spellcheck="false"
          :placeholder="$t('sidePanel.server.placeholder')"
          :aria-label="$t('sidePanel.sec.server')"
          @blur="saveServerCommand"
          @keydown.enter="saveServerCommand"
        />
        <p class="sp-empty">{{ $t("sidePanel.server.hint") }}</p>
        <p v-if="serverMsg" class="sp-empty">{{ serverMsg }}</p>
      </div>
    </section>

    <!-- Anything -->
    <section v-if="session?.anything?.enabled" class="sp-section">
      <button class="sp-head" @click="toggle('anything')">
        <span class="msi chevron">{{ collapsed.anything ? "chevron_right" : "expand_more" }}</span>
        <span class="sp-title">{{ $t("sidePanel.sec.anything") }}</span>
        <span v-if="anythingTargets.length" class="sp-count">{{ anythingTargets.length }}</span>
      </button>
      <div v-if="!collapsed.anything" class="sp-body">
        <div class="sp-create">
          <button class="sp-btn" @click="anythingSetupVisible = true">{{ $t("anythingSetup.open") }}</button>
        </div>
        <div class="sp-create">
          <button class="sp-btn" :disabled="anythingBusy" @click="anythingAdd(true)">{{ $t("sidePanel.anything.addFolder") }}</button>
          <button class="sp-btn" :disabled="anythingBusy" @click="anythingAdd(false)">{{ $t("sidePanel.anything.addFile") }}</button>
        </div>
        <div v-for="f in anythingSources" :key="f.caminho" class="sp-row" :title="f.caminho">
          <span class="row-text">{{ f.tipo === "pasta" ? "📁" : "📄" }} {{ f.caminho }}</span>
          <button class="sp-btn" :title="$t('sidePanel.anything.remove')" @click="anythingRemove(f.caminho)">✕</button>
        </div>
        <p v-if="!anythingSources.length" class="sp-empty">{{ $t("sidePanel.anything.noSources") }}</p>
        <div class="sp-create">
          <button class="sp-btn" :disabled="anythingBusy" @click="anythingScan">{{ $t("sidePanel.anything.scan") }}</button>
          <button class="sp-btn" :disabled="anythingBusy || !anythingTargets.length" @click="anythingPlan">{{ $t("sidePanel.anything.plan") }}</button>
        </div>
        <p v-if="anythingMsg" class="sp-empty">{{ anythingMsg }}</p>
        <p v-if="!anythingTargets.length" class="sp-empty">{{ $t("sidePanel.anything.empty") }}</p>
        <div v-for="a in anythingTargets" :key="a.caminho" class="sp-row" :title="a.caminho + ' (' + a.suporte + ')'">
          <span class="row-text">{{ a.caminho }}</span>
          <span class="row-status">{{ a.formato }} · {{ anythingStatusLabel(a.status) }}</span>
        </div>
        <details class="sp-final">
          <summary>{{ $t("anythingFinal.sessionTitle") }}</summary>
          <AnythingFinalPrompt
            :value="session?.anything?.final_prompt || ''"
            :fallback="anythingFinalFallback"
            :reset-label="$t('anythingFinal.useGlobal')"
            @save="saveSessionFinal"
          />
        </details>
        <p v-if="anythingIgnored" class="sp-empty">{{ $t("sidePanel.anything.ignored", { n: anythingIgnored }) }}</p>
      </div>
    </section>

    <AnythingSetupDialog v-model:visible="anythingSetupVisible" />

    <!-- Git -->
    <section class="sp-section">
      <button class="sp-head" @click="toggle('git')">
        <span class="msi chevron">{{ collapsed.git ? "chevron_right" : "expand_more" }}</span>
        <span class="sp-title">{{ $t("sidePanel.sec.git") }}</span>
        <span v-if="gitChanges.length" class="sp-count">{{ gitChanges.length }}</span>
      </button>
      <div v-if="!collapsed.git" class="sp-body">
        <p v-if="!gitRoot" class="sp-empty">{{ $t("sidePanel.git.noProject") }}</p>
        <p v-else-if="gitError === 'not-repo'" class="sp-empty">{{ $t("sidePanel.git.notRepo") }}</p>
        <p v-else-if="!gitChanges.length" class="sp-empty">{{ $t("sidePanel.git.none") }}</p>
        <div v-for="f in gitChanges" :key="f.path" class="sp-row" :title="f.path">
          <span class="git-status" :class="f.status">{{ f.status[0].toUpperCase() }}</span>
          <span class="row-text">{{ f.path }}</span>
          <span class="git-stat"><span class="add">+{{ f.additions }}</span> <span class="del">-{{ f.deletions }}</span></span>
        </div>
      </div>
    </section>

    <!-- Processos em segundo plano -->
    <section class="sp-section">
      <button class="sp-head" @click="toggle('jobs')">
        <span class="msi chevron">{{ collapsed.jobs ? "chevron_right" : "expand_more" }}</span>
        <span class="sp-title">{{ $t("sidePanel.sec.jobs") }}</span>
        <span class="sp-count">{{ runningJobs ? `${runningJobs}/${jobs.length}` : jobs.length }}</span>
      </button>
      <div v-if="!collapsed.jobs" class="sp-body">
        <div v-for="j in jobs" :key="j.id" class="sp-row clickable" :title="j.command" @click="selectedJobId = j.id">
          <span class="status-dot" :class="j.status.kind === 'running' ? 'running' : 'idle'"></span>
          <span class="row-text mono">{{ j.command }}</span>
          <span class="row-status">{{ statusLabel(j.status.kind) }}</span>
        </div>
        <p v-if="!jobs.length" class="sp-empty">{{ $t("sidePanel.jobs.none") }}</p>
      </div>
    </section>

    <ProcessHistoryModal v-if="selectedJob" :job="selectedJob" @close="selectedJobId = null" />

    <!-- Agentes -->
    <section class="sp-section">
      <button class="sp-head" @click="toggle('agents')">
        <span class="msi chevron">{{ collapsed.agents ? "chevron_right" : "expand_more" }}</span>
        <span class="sp-title">{{ $t("sidePanel.sec.agents") }}</span>
        <span class="sp-count">{{ runningAgents ? `${runningAgents}/${executions.length}` : executions.length }}</span>
      </button>
      <div v-if="!collapsed.agents" class="sp-body">
        <div v-for="e in executions" :key="e.id" class="sp-row" :title="e.name">
          <span class="status-dot" :class="e.status === 'running' ? 'running' : e.status === 'failed' ? 'failed' : 'ok'"></span>
          <span class="row-text">{{ e.name }}</span>
          <span class="row-status">{{ statusLabel(e.status) }}</span>
        </div>
        <p v-if="!executions.length" class="sp-empty">{{ $t("sidePanel.agents.none") }}</p>
      </div>
    </section>

    <!-- Sub-sessões -->
    <section class="sp-section">
      <button class="sp-head" @click="toggle('sub')">
        <span class="msi chevron">{{ collapsed.sub ? "chevron_right" : "expand_more" }}</span>
        <span class="sp-title">{{ $t("sidePanel.sec.sub") }}</span>
        <span class="sp-count">{{ children.length }}</span>
      </button>
      <div v-if="!collapsed.sub" class="sp-body">
        <div v-if="parent" class="sp-row clickable" @click="openSession(parent.id)">
          <span class="msi row-icon">arrow_upward</span>
          <span class="row-text">{{ $t("sidePanel.sub.parent") }}: {{ parent.title }}</span>
        </div>
        <div v-for="c in children" :key="c.id" class="sp-row clickable" @click="openSession(c.id)">
          <span class="msi row-icon">subdirectory_arrow_right</span>
          <span class="row-text">{{ c.title }}</span>
          <span class="row-status">{{ $t("sidePanel.sub.open") }}</span>
        </div>
        <p v-if="!children.length && !parent" class="sp-empty">{{ $t("sidePanel.sub.none") }}</p>
      </div>
    </section>
  </aside>
</template>

<style scoped>
.side-panel {
  width: 280px;
  flex-shrink: 0;
  height: 100%;
  overflow-y: auto;
  border-left: var(--cerne-border);
  background: var(--cerne-surface, transparent);
  font-size: 13px;
}

.sp-section {
  border-bottom: var(--cerne-border);
}

.sp-head {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 10px 12px;
  background: none;
  border: none;
  color: inherit;
  cursor: pointer;
  font: inherit;
  font-weight: 600;
  text-align: left;
}

.sp-head .chevron {
  font-size: 18px;
  opacity: 0.6;
}

.sp-title {
  flex: 1;
}

.sp-count {
  font-size: 11px;
  opacity: 0.6;
  font-weight: 500;
}

.sp-body {
  padding: 0 12px 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.sp-empty {
  margin: 0;
  opacity: 0.6;
  font-size: 12px;
}

.sp-select,
.sp-input {
  width: 100%;
  box-sizing: border-box;
  padding: 4px 6px;
  font: inherit;
  font-size: 13px;
  border: 1px solid var(--border, #d0d7de);
  border-radius: 6px;
  background: transparent;
  color: inherit;
}

.sp-create {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-top: 8px;
}

.sp-btn {
  padding: 4px 8px;
  font: inherit;
  font-size: 13px;
  border: 1px solid var(--border, #d0d7de);
  border-radius: 6px;
  background: transparent;
  cursor: pointer;
}

.sp-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.sp-row {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.sp-row.clickable {
  cursor: pointer;
  border-radius: 6px;
  padding: 4px 6px;
  margin: 0 -6px;
}

.sp-row.clickable:hover {
  background: rgba(127, 127, 127, 0.12);
}

.row-icon {
  font-size: 16px;
  opacity: 0.7;
}

.row-main {
  display: flex;
  flex-direction: column;
  min-width: 0;
  gap: 2px;
}

.row-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
  flex: 1;
}

.row-text.mono {
  font-family: ui-monospace, Consolas, monospace;
  font-size: 12px;
}

.row-status {
  font-size: 11px;
  opacity: 0.6;
  white-space: nowrap;
}

.row-badge {
  font-size: 10px;
  padding: 1px 6px;
  border-radius: 999px;
  width: fit-content;
  opacity: 0.85;
}

.row-badge.write {
  background: rgba(34, 197, 94, 0.15);
  color: #16a34a;
}

.row-badge.read {
  background: rgba(127, 127, 127, 0.18);
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
  background: #a1a1aa;
}

.status-dot.ok {
  background: #22c55e;
}

.status-dot.running {
  background: #3b82f6;
  animation: sp-pulse 1.2s ease-in-out infinite;
}

.status-dot.failed {
  background: #ef4444;
}

@keyframes sp-pulse {
  50% {
    opacity: 0.3;
  }
}

.git-status {
  width: 16px;
  text-align: center;
  font-weight: 700;
  font-size: 11px;
  flex-shrink: 0;
}

.git-status.added,
.git-status.untracked {
  color: #16a34a;
}

.git-status.deleted {
  color: #ef4444;
}

.git-status.modified,
.git-status.renamed {
  color: #d97706;
}

.git-stat {
  font-size: 11px;
  white-space: nowrap;
}

.git-stat .add {
  color: #16a34a;
}

.git-stat .del {
  color: #ef4444;
}
</style>
