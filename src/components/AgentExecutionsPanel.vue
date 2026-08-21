<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { UnlistenFn } from "@tauri-apps/api/event";
import Dialog from "primevue/dialog";
import { api, onChatToken, onThinkingToken, type AgentExecution } from "../api";
import { useSessionStore } from "../stores/session";
import TaskStepGroup from "./TaskStepGroup.vue";

// Fase C2 do roteiro de Agentes/Skills: painel pra acompanhar execuções de
// `task`/`verify_completion` da sessão atual em tempo real, SOMENTE LEITURA
// — sem input do usuário nem `ask` chegando até aqui (decisão de design:
// sub-execuções não podem pausar esperando o usuário, senão travam a fila).
// Cancelamento individual por execução NÃO é suportado ainda: hoje elas
// rodam dentro do mesmo turno da sessão pai, sem um handle próprio de abort
// — só dá pra cancelar o turno inteiro (botão de parar do composer). Fica
// como gap documentado pra uma iteração futura.

const { t } = useI18n();
const sessionStore = useSessionStore();

const executions = ref<AgentExecution[]>([]);
const panelOpen = ref(false);
let poll: ReturnType<typeof setInterval> | null = null;

async function refresh() {
  try {
    const all = await api.listAgentExecutions();
    executions.value = all.filter((e) => e.session_id === sessionStore.currentId);
  } catch {
    executions.value = [];
  }
  syncPolling();
}

// Sem push de mudança de status (running->done/failed) hoje — poll cobre,
// mas só enquanto tiver algo `running` de verdade: senão ficava chamando
// `list_agent_executions` pra sempre (a cada 3s, em toda sessão aberta,
// mesmo sem nenhuma execução ativa) — achado testando ao vivo, 2026-08-19,
// via aba Rede do DevTools. Cada `refresh()` decide se liga/desliga o
// timer sozinho, então o poll começa quando a 1ª execução `running`
// aparece e para assim que a última termina.
function syncPolling() {
  const active = executions.value.some((e) => e.status === "running");
  if (active && !poll) {
    poll = setInterval(refresh, 3000);
  } else if (!active && poll) {
    clearInterval(poll);
    poll = null;
  }
}

onMounted(refresh);
onUnmounted(() => {
  if (poll) clearInterval(poll);
  closeDetail();
});
watch(() => sessionStore.currentId, refresh);

const runningCount = computed(() => executions.value.filter((e) => e.status === "running").length);

const KIND_ICONS: Record<string, string> = {
  task: "smart_toy",
  verify_completion: "fact_check",
  pipeline: "sync_alt",
};

function statusLabel(status: AgentExecution["status"]): string {
  return t(`agentExecutions.status.${status}`);
}

// `detailId` (não o objeto inteiro) pra `detail` continuar refletindo o
// poll de `executions` a cada 3s — assim os passos (`steps`, gravados no
// backend desde T14/Fase 3, ver TaskStepGroup.vue) continuam atualizando
// mesmo com o modal já aberto, e ficam visíveis depois que a execução
// termina (achado testando ao vivo, 2026-08-16: antes o modal ficava vazio
// pra execução já concluída, só mostrava algo enquanto rodava ao vivo).
const detailId = ref<string | null>(null);
const detail = computed(() => executions.value.find((e) => e.id === detailId.value) ?? null);

function childExecutions(id: string): AgentExecution[] {
  return executions.value
    .filter((e) => e.parent_id === id)
    .sort((a, b) => a.started_at_ms - b.started_at_ms);
}

const liveText = ref("");
let unlistenToken: UnlistenFn | null = null;
let unlistenThinking: UnlistenFn | null = null;

async function openDetail(exec: AgentExecution) {
  closeDetail();
  detailId.value = exec.id;
  liveText.value = "";
  // Mesmo canal sintético que subagent.rs/verifier.rs usam pra streamar
  // (ver Fase A2) — nunca vaza pro chat:token da sessão real.
  const channel = `${exec.session_id}::exec::${exec.id}`;

  unlistenToken = await onChatToken((sid, delta) => {
    if (sid === channel) liveText.value += delta;
  });
  unlistenThinking = await onThinkingToken((sid, delta) => {
    if (sid === channel) liveText.value += delta;
  });
}

function closeDetail() {
  detailId.value = null;
  unlistenToken?.();
  unlistenThinking?.();
  unlistenToken = unlistenThinking = null;
}
</script>

<template>
  <div v-if="executions.length > 0" class="ae-panel">
    <button class="ae-toggle" @click="panelOpen = !panelOpen">
      <span class="msi">smart_toy</span>
      {{ $t("agentExecutions.count", { count: executions.length, running: runningCount }) }}
      <span class="msi ae-chevron" :class="{ open: panelOpen }">expand_more</span>
    </button>
    <div v-if="panelOpen" class="ae-list">
      <div v-for="exec in executions" :key="exec.id" class="ae-row" @click="openDetail(exec)">
        <span class="msi ae-icon" :class="exec.status">{{ KIND_ICONS[exec.kind] ?? "extension" }}</span>
        <span class="ae-name">{{ exec.name }}</span>
        <span class="ae-status">{{ statusLabel(exec.status) }}</span>
      </div>
    </div>
  </div>

  <Dialog
    :visible="!!detail"
    @update:visible="(v) => { if (!v) closeDetail(); }"
    :header="detail?.name"
    modal
    :style="{ width: '640px', maxWidth: '92vw' }"
  >
    <p class="ae-detail-hint">{{ $t("agentExecutions.readOnlyHint") }}</p>
    <template v-if="detail">
      <div v-for="child in childExecutions(detail.id)" :key="child.id" class="ae-child-group">
        <div class="ae-child-header">
          <span class="msi ae-icon" :class="child.status">{{ KIND_ICONS[child.kind] ?? "extension" }}</span>
          {{ child.name }} — {{ statusLabel(child.status) }}
        </div>
        <TaskStepGroup :tasks="child.steps" />
      </div>
      <TaskStepGroup v-if="detail.steps.length > 0" :tasks="detail.steps" />
    </template>
    <pre v-if="liveText" class="ae-text">{{ liveText }}</pre>
    <p v-else-if="detail && detail.steps.length === 0 && childExecutions(detail.id).length === 0" class="hint">
      {{ $t("agentExecutions.waiting") }}
    </p>
  </Dialog>
</template>

<style scoped>
.ae-panel {
  margin: 0 0 6px;
  min-width: 0;
  max-width: 100%;
}

.ae-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 100%;
  border: var(--cerne-border);
  background: #fafafa;
  border-radius: 8px;
  padding: 5px 10px;
  font-size: 12px;
  font-weight: 500;
  color: #52525b;
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ae-toggle .msi {
  font-size: 15px;
}

.ae-chevron {
  transition: transform 0.15s ease;
  font-size: 16px !important;
  color: #a1a1aa;
}

.ae-chevron.open {
  transform: rotate(180deg);
}

.ae-list {
  margin-top: 4px;
  border: var(--cerne-border);
  border-radius: 8px;
  max-width: 100%;
  max-height: 280px;
  overflow-y: auto;
  overflow-x: hidden;
}

.ae-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  font-size: 12px;
  cursor: pointer;
  border-bottom: 1px solid #f4f4f5;
  min-width: 0;
  max-width: 320px;
}

.ae-row:last-child {
  border-bottom: none;
}

.ae-row:hover {
  background: #f4f4f5;
}

.ae-icon {
  font-size: 15px;
  color: #a1a1aa;
  flex-shrink: 0;
}

.ae-icon.running {
  color: #3f3f46;
  animation: ae-spin 1s linear infinite;
}

.ae-icon.done {
  color: #16a34a;
}

.ae-icon.failed {
  color: #dc2626;
}

@keyframes ae-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.ae-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  color: #18181b;
}

.ae-status {
  flex-shrink: 0;
  color: #71717a;
}

.ae-detail-hint {
  margin: 0 0 10px;
  font-size: 11px;
  font-weight: 500;
  color: #a1a1aa;
}

.ae-child-group {
  margin-bottom: 10px;
  padding-bottom: 6px;
  border-bottom: 1px solid #f4f4f5;
}

.ae-child-group:last-child {
  border-bottom: none;
}

.ae-child-header {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  color: #3f3f46;
  margin-bottom: 4px;
}

.ae-text {
  margin: 0;
  max-height: 320px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 12px;
  font-family: ui-monospace, monospace;
  background: #fafafa;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 10px 12px;
}

.hint {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
}
</style>
