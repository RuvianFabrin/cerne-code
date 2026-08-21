<script setup lang="ts">
import { computed, nextTick, watch, ref, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import { useSessionStore } from "../stores/session";
import MessageBubble from "./MessageBubble.vue";
import MarkdownContent from "./MarkdownContent.vue";
import ComposerBar from "./ComposerBar.vue";
import DiffReview from "./DiffReview.vue";
import AskCard from "./AskCard.vue";
import PermissionCard from "./PermissionCard.vue";
import AgentsSkillsPlanCard from "./AgentsSkillsPlanCard.vue";
import BackgroundJobsPanel from "./BackgroundJobsPanel.vue";
import AgentExecutionsPanel from "./AgentExecutionsPanel.vue";
import TaskStepGroup from "./TaskStepGroup.vue";
import TodoCard from "./TodoCard.vue";
import { formatElapsed, formatTokens } from "../taskLabels";
import type { ChatMessage, TaskItem, TurnStats } from "../api";

defineEmits<{ "open-settings": [] }>();

const { t } = useI18n();
const sessionStore = useSessionStore();
const scrollRef = ref<HTMLDivElement | null>(null);

// Auto-stick no fundo do chat: gruda no final enquanto o usuário não rolar
// pra cima. Assim que ele rolar, para de forçar o scroll (pra poder ler
// mensagens antigas em paz); volta a grudar quando ele manda a próxima
// mensagem ou clica no botão "ir pro final".
const NEAR_BOTTOM_PX = 80;
const autoStick = ref(true);
const showJumpToBottom = ref(false);
let lastMessageCount = 0;

function isNearBottom(): boolean {
  const el = scrollRef.value;
  if (!el) return true;
  return el.scrollHeight - el.scrollTop - el.clientHeight <= NEAR_BOTTOM_PX;
}

function onChatScroll() {
  autoStick.value = isNearBottom();
  showJumpToBottom.value = !autoStick.value;
}

function scrollToBottom(behavior: ScrollBehavior = "auto") {
  scrollRef.value?.scrollTo({ top: scrollRef.value.scrollHeight, behavior });
}

function jumpToBottom() {
  autoStick.value = true;
  showJumpToBottom.value = false;
  scrollToBottom("smooth");
}

const now = ref(Date.now());
let tickTimer: ReturnType<typeof setInterval> | null = null;
onMounted(() => { tickTimer = setInterval(() => { now.value = Date.now(); }, 1000); });
onUnmounted(() => { if (tickTimer) clearInterval(tickTimer); });

// Timeline do chat: mensagem do usuário -> passos de ferramenta daquele
// turno (rótulo amigável, expansível) -> resposta final do agente. As
// mensagens de assistant sem conteúdo (só tool_calls, sem texto) não viram
// bolha vazia — o passo já aparece representado no grupo de steps.
type TimelineItem =
  | { kind: "message"; key: string; message: ChatMessage }
  | { kind: "steps"; key: string; tasks: TaskItem[] }
  | { kind: "todo"; key: string; todos: import("../api").TodoItem[] }
  | { kind: "stats"; key: string; stats: TurnStats }
  | { kind: "background-note"; key: string; message: ChatMessage };

const timeline = computed<TimelineItem[]>(() => {
  const items: TimelineItem[] = [];
  let userTurn = 0;
  let todoIdx = 0;
  let taskOffset = 0;
  const allTasks = sessionStore.tasks;
  const stats = sessionStore.turnStats;
  sessionStore.messages.forEach((m, i) => {
    if (m.role === "user") {
      if (userTurn > 0 && stats[userTurn]) {
        items.push({ kind: "stats", key: `stats-${userTurn}`, stats: stats[userTurn] });
      }
      userTurn++;
      taskOffset = allTasks.findIndex((t) => t.turn === userTurn);
      if (taskOffset < 0) taskOffset = allTasks.length;
      items.push({ kind: "message", key: `m-${i}`, message: m });
    } else if (m.role === "assistant") {
      const tcCount = m.tool_calls?.length ?? 0;
      if (tcCount > 0) {
        const batch = allTasks.slice(taskOffset, taskOffset + tcCount);
        taskOffset += tcCount;
        if (batch.length > 0) {
          items.push({ kind: "steps", key: `s-${i}`, tasks: batch });
          const todoCount = batch.filter((t) => t.label.startsWith("todo_list(")).length;
          for (let j = 0; j < todoCount && todoIdx < sessionStore.todoSnapshots.length; j++) {
            items.push({ kind: "todo", key: `t-${todoIdx}`, todos: sessionStore.todoSnapshots[todoIdx].todos });
            todoIdx++;
          }
        }
      }
      const hasText = (m.display_content ?? m.content)?.trim();
      if (hasText) {
        items.push({ kind: "message", key: `m-${i}`, message: m });
      }
    } else if (m.role === "system" && m.name === "background_job_done") {
      // T14: nota de conclusão de job em segundo plano — role "system" pra
      // não entrar na conversa como se o usuário/agente tivesse "dito"
      // aquilo, mas com um marcador (`name`) pra diferenciar do system
      // prompt real (que também é role "system", mas nunca deveria
      // aparecer aqui — só essa mensagem específica é intencionalmente
      // visível).
      items.push({ kind: "background-note", key: `bn-${i}`, message: m });
    }
  });
  if (userTurn > 0 && stats[userTurn]) {
    items.push({ kind: "stats", key: `stats-${userTurn}`, stats: stats[userTurn] });
  }
  return items;
});

// Marcador por mensagem do usuário na lateral do chat — pedido do usuário
// (2026-08-20, viu algo parecido na própria interface do Claude Code e
// perguntou se dava pra fazer aqui também). Uma tracinho por mensagem SUA
// (não do assistente), distribuídos uniformemente numa faixa fina fixada
// na borda esquerda; clicar rola até aquela mensagem. `messageEls` guarda
// o elemento DOM raiz de cada `MessageBubble` (via template ref funcional —
// `.$el` funciona mesmo sem `defineExpose`, é propriedade nativa do Vue,
// não um binding exposto pelo componente) indexado pela `key` do item na
// timeline, pra não precisar de outro array/estado duplicado.
const userMessageMarkers = computed(() =>
  timeline.value.filter((item): item is Extract<TimelineItem, { kind: "message" }> => item.kind === "message" && item.message.role === "user"),
);
const messageEls = new Map<string, HTMLElement>();
function registerMessageEl(key: string, el: unknown) {
  if (!el) {
    messageEls.delete(key);
    return;
  }
  const domEl = (el as { $el?: HTMLElement }).$el ?? (el as HTMLElement);
  if (domEl instanceof HTMLElement) messageEls.set(key, domEl);
}
function scrollToMessage(key: string) {
  messageEls.get(key)?.scrollIntoView({ behavior: "smooth", block: "start" });
}

// 2026-08-17, pedido do usuário: avisar quando `git` não está instalado
// nesta máquina, já que o visualizador de diff de repositório (T42) depende
// dele — só mostra quando de fato importa (sessão tem pasta de projeto),
// não em toda sessão de chat puro.
const showGitWarning = computed(
  () =>
    sessionStore.gitAvailable === false &&
    !!sessionStore.currentSession?.project_root &&
    !sessionStore.gitWarningDismissed,
);

const statusLabel = computed(() => {
  // Fase 3: enquanto um pipeline Dev→QA→Analista está rodando, a etapa/round
  // atual é mais informativa que "pensando"/"rodando ferramenta" genérico —
  // tem prioridade sobre o resto enquanto `pipelineStatus` estiver setado.
  if (sessionStore.pipelineStatus) {
    const p = sessionStore.pipelineStatus;
    return t(`chat.pipelineStep.${p.step}`, { round: p.round, maxRounds: p.max_rounds });
  }
  if (sessionStore.status === "starting_server") return t("chat.startingServer");
  if (sessionStore.status === "compacting") return t("chat.compactingContext");
  if (sessionStore.status === "thinking") {
    // Nem todo provider/modelo transmite tokens de raciocínio visíveis
    // (thinkingText) — sem isso, "Pensando..." parado por minutos parece
    // travado mesmo não estando. Enquanto não chegou nenhum token de
    // raciocínio nem de resposta, deixa claro que é o modelo processando
    // (TTFT), não uma sessão de "pensamento" visível.
    const hasVisibleReasoning = !!sessionStore.thinkingText;
    const base = hasVisibleReasoning ? t("chat.thinking") : t("chat.awaitingModel");
    if (sessionStore.thinkingStartedAt) {
      return `${base} ${formatElapsed(now.value - sessionStore.thinkingStartedAt)}`;
    }
    return base;
  }
  if (sessionStore.status === "running_tool") return sessionStore.activeToolLabel;
  return "";
});

const thinkingTail = computed(() => {
  const t = sessionStore.thinkingText;
  if (!t) return "";
  const tail = t.length > 200 ? "..." + t.slice(-200) : t;
  return tail.replace(/\n/g, " ").trim();
});

watch(
  () => [
    sessionStore.messages.length,
    sessionStore.streamingText,
    sessionStore.thinkingText,
    sessionStore.tasks.length,
    sessionStore.pendingQuestion,
  ],
  async () => {
    const count = sessionStore.messages.length;
    // Mensagem nova do usuário (ele mandou algo) sempre volta a grudar no
    // final, mesmo que ele tivesse rolado pra cima lendo o histórico.
    if (count > lastMessageCount && sessionStore.messages[count - 1]?.role === "user") {
      autoStick.value = true;
      showJumpToBottom.value = false;
    }
    lastMessageCount = count;
    await nextTick();
    if (autoStick.value) scrollToBottom();
  },
);

// Trocar de sessão (ou abrir uma com histórico) sempre começa grudado no
// final — sem isso o scroll ficava em 0 e parecia que "tudo" estava
// amontoado no topo até o usuário rolar manualmente.
watch(
  () => sessionStore.currentId,
  async () => {
    autoStick.value = true;
    showJumpToBottom.value = false;
    lastMessageCount = sessionStore.messages.length;
    await nextTick();
    scrollToBottom();
  },
  { immediate: true },
);
</script>

<template>
  <div v-if="!sessionStore.currentId" class="empty-state">
    <span class="msi big">graphic_eq</span>
    <p>{{ $t("chat.emptyState") }}</p>
  </div>
  <template v-else>
    <div class="chat-layout">
      <div class="chat-column">
        <!-- Alvos de Teleport pro toolbar que hoje mora em ComposerBar.vue
             (seletor de modelo/visão à esquerda; navegar arquivos/diff à
             direita) — pedido do usuário (2026-08-20): tirar esses controles
             de dentro da caixa do composer e subir pro topo da área de chat.
             A lógica/estado continuam 100% em ComposerBar.vue, só a posição
             visual muda (CSS scoped viaja junto com o Teleport). -->
        <div class="chat-topbar">
          <div id="chat-topbar-left" class="topbar-left"></div>
          <div id="chat-topbar-right" class="topbar-right"></div>
        </div>
        <div class="chat-scroll-wrap">
          <!-- Marcador por mensagem sua, na borda esquerda — clique rola até
               a mensagem (pedido do usuário, 2026-08-20). Pilha compacta
               (gap fixo, um logo abaixo do outro), não espalhada
               proporcionalmente pela altura toda — achado testando ao vivo:
               espalhado deixava as marcações longe umas das outras sem
               motivo, com poucas mensagens quase saindo da tela. Só aparece
               com 2+ mensagens (com 1 só não tem o que navegar). -->
          <div v-if="userMessageMarkers.length > 1" class="message-markers">
            <button
              v-for="item in userMessageMarkers"
              :key="item.key"
              class="message-marker"
              v-tooltip.right="(item.message.display_content ?? item.message.content).slice(0, 80)"
              @click="scrollToMessage(item.key)"
            />
          </div>
          <div class="chat-scroll" ref="scrollRef" @scroll="onChatScroll">
            <div class="chat-inner">
            <template v-for="item in timeline" :key="item.key">
              <div v-if="item.kind === 'message'" :ref="item.message.role === 'user' ? (el) => registerMessageEl(item.key, el) : undefined">
                <MessageBubble :message="item.message" />
              </div>
              <TaskStepGroup v-else-if="item.kind === 'steps'" :tasks="item.tasks" />
              <TodoCard v-else-if="item.kind === 'todo'" :todos="item.todos" />
              <div v-else-if="item.kind === 'stats'" class="turn-stats">
                <span class="msi">schedule</span>
                {{ formatElapsed(item.stats.elapsed_ms) }}
                <span class="stats-sep">·</span>
                {{ $t("chat.tokens", { count: formatTokens(item.stats.prompt_tokens + item.stats.completion_tokens) }) }}
              </div>
              <div v-else-if="item.kind === 'background-note'" class="background-note">
                <MarkdownContent :content="item.message.display_content ?? item.message.content" />
              </div>
            </template>
            <!-- Blocos do turno em andamento, na ordem real em que texto e
                 chamadas de ferramenta aconteceram (ver liveBlocks em
                 session.ts) — sem isso tudo aparecia agrupado (steps
                 primeiro, texto todo concatenado depois) até o turno
                 terminar e recarregar do histórico. -->
            <template v-for="block in sessionStore.liveBlocks" :key="block.id">
              <TaskStepGroup v-if="block.kind === 'tools'" :tasks="block.tasks" />
              <div v-else class="row">
                <div class="bubble streaming">
                  <MarkdownContent :content="block.text" />
                </div>
              </div>
            </template>
            <div v-if="sessionStore.showComputerUseWarning" class="computer-use-warning">
              <span class="msi">warning</span>
              <div class="warning-text">
                <strong>{{ $t("chat.computerUseWarningTitle") }}</strong>
                <p>{{ $t("chat.computerUseWarningBody") }}</p>
              </div>
              <button class="warning-dismiss" @click="sessionStore.showComputerUseWarning = false">{{ $t("chat.gotIt") }}</button>
            </div>
            <div v-if="showGitWarning" class="computer-use-warning">
              <span class="msi">warning</span>
              <div class="warning-text">
                <strong>{{ $t("chat.gitMissingWarningTitle") }}</strong>
                <p>{{ $t("chat.gitMissingWarningBody") }}</p>
              </div>
              <button class="warning-dismiss" @click="sessionStore.gitWarningDismissed = true">{{ $t("chat.gotIt") }}</button>
            </div>
            <div v-if="statusLabel" class="status-line">
              <span class="msi spin">progress_activity</span>
              {{ statusLabel }}
            </div>
            <div v-if="sessionStore.thinkingText" class="thinking-preview">
              <span class="msi spin">psychology</span>
              <span class="thinking-text">{{ thinkingTail }}</span>
            </div>
            <div v-if="sessionStore.lastCompactionNote" class="compaction-note">
              <span class="msi">compress</span>
              {{ sessionStore.lastCompactionNote }}
            </div>
            <p v-if="sessionStore.error" class="error-line">{{ sessionStore.error }}</p>
          </div>
          </div>
        </div>
        <div class="composer-wrap">
          <button v-if="showJumpToBottom" class="jump-to-bottom" @click="jumpToBottom" :title="$t('chat.jumpToBottom')">
            <span class="msi">arrow_downward</span>
          </button>
          <!-- Tudo que pede uma ação do usuário (aceitar edição, aprovar
               ferramenta em modo Manual, responder pergunta) fica ancorado
               aqui, fora da área de scroll — enterrado lá no topo, junto
               com o histórico, passava despercebido. -->
          <div v-if="sessionStore.pendingEdits.length > 0" class="pending-edits-anchor">
            <DiffReview v-for="edit in sessionStore.pendingEdits" :key="edit.id" :edit="edit" />
          </div>
          <AgentsSkillsPlanCard />
          <PermissionCard />
          <AskCard />
          <ComposerBar />
        </div>
        <!-- T40: painéis de status (jobs em segundo plano, execuções de
             agente/skill) ficam FORA do fluxo do composer, numa área
             flutuante — dentro do composer-wrap eles empilhavam e cobriam
             o chat quando havia vários de uma vez. -->
        <div class="floating-status">
          <BackgroundJobsPanel />
          <AgentExecutionsPanel />
        </div>
      </div>
    </div>
  </template>
</template>

<style scoped>
.empty-state {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  color: #a1a1aa;
}

.empty-state .msi.big {
  font-size: 40px;
}

.chat-layout {
  flex: 1;
  display: flex;
  min-height: 0;
  position: relative;
}

.message-markers {
  position: absolute;
  left: 4px;
  top: 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  z-index: 15;
}

.message-marker {
  width: 10px;
  height: 3px;
  border: none;
  border-radius: 999px;
  background: #d4d4d8;
  cursor: pointer;
  padding: 0;
  flex-shrink: 0;
}

.message-marker:hover {
  background: #6366f1;
  width: 16px;
}

/* T40: painéis de status flutuantes (background jobs, execuções de
   agente/skill) — canto superior direito, fora do fluxo do composer. */
.floating-status {
  position: absolute;
  top: 12px;
  right: 16px;
  z-index: 20;
  display: flex;
  flex-direction: column;
  gap: 6px;
  max-width: min(320px, calc(100vw - 32px));
  max-height: calc(100% - 24px);
  overflow-y: auto;
  align-items: flex-end;
}

.chat-column {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.chat-scroll-wrap {
  position: relative;
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
}

.chat-scroll {
  flex: 1;
  min-height: 0;
  overflow-y: auto;
  scroll-padding-bottom: 24px;
}

.chat-inner {
  max-width: 960px;
  margin: 0 auto;
  padding: 24px 24px 24px;
}

.composer-wrap {
  position: relative;
  max-width: 960px;
  width: 100%;
  margin: 0 auto;
  padding: 0 24px 20px;
}

.chat-topbar {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  max-width: 960px;
  width: 100%;
  margin: 0 auto;
  padding: 12px 24px;
  border-bottom: var(--cerne-border);
}

.topbar-left,
.topbar-right {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.jump-to-bottom {
  position: absolute;
  top: -44px;
  left: 50%;
  transform: translateX(-50%);
  display: flex;
  align-items: center;
  justify-content: center;
  width: 34px;
  height: 34px;
  border-radius: 50%;
  border: var(--cerne-border);
  background: #ffffff;
  color: #3f3f46;
  cursor: pointer;
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.12);
  z-index: 2;
}

.jump-to-bottom:hover {
  background: #f4f4f5;
}

.jump-to-bottom .msi {
  font-size: 18px;
}

.pending-edits-anchor {
  max-height: 40vh;
  overflow-y: auto;
}

.row {
  display: flex;
  padding: 4px 0;
}

.bubble.streaming {
  max-width: 72ch;
  padding: 4px 0;
}

.status-line {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 400;
  color: #71717a;
  padding: 6px 2px;
}

.status-line .spin {
  font-size: 15px;
  animation: spin 1s linear infinite;
}

.thinking-preview {
  display: flex;
  align-items: flex-start;
  gap: 6px;
  font-size: 12px;
  font-weight: 400;
  color: #a1a1aa;
  padding: 2px 2px 6px;
  font-style: italic;
}

.thinking-preview .spin {
  font-size: 14px;
  animation: spin 2s linear infinite;
  flex-shrink: 0;
  margin-top: 1px;
}

.thinking-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  max-width: 60ch;
}

.error-line {
  font-size: 12px;
  font-weight: 400;
  color: #dc2626;
  padding: 6px 2px;
}

/* T14: nota de conclusão de job em segundo plano, injetada sozinha no
   histórico — precisa ser claramente diferente de uma bolha de chat normal
   (não foi "dito" por ninguém), então vira um card discreto em vez de um
   balão de mensagem. */
.background-note {
  max-width: 72ch;
  border: 1px dashed #d4d4d8;
  border-radius: 10px;
  padding: 8px 12px;
  margin: 6px 0;
  font-size: 13px;
  color: #3f3f46;
  background: #fafafa;
}

.compaction-note {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 11px;
  font-weight: 400;
  color: #71717a;
  padding: 4px 2px;
}

.compaction-note .msi {
  font-size: 14px;
}

.turn-stats {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  font-weight: 400;
  color: #b0b0b8;
  padding: 8px 2px 2px;
  font-family: ui-monospace, monospace;
}

.turn-stats .msi {
  font-size: 13px;
}

.stats-sep {
  color: #d4d4d8;
}

.computer-use-warning {
  display: flex;
  align-items: flex-start;
  gap: 10px;
  background: #fffbeb;
  border: 1px solid #fde68a;
  border-radius: 10px;
  padding: 10px 14px;
  margin: 6px 0;
  font-size: 12px;
  color: #92400e;
}

.computer-use-warning .msi {
  font-size: 18px;
  color: #d97706;
  flex-shrink: 0;
  margin-top: 1px;
}

.warning-text {
  flex: 1;
  min-width: 0;
}

.warning-text strong {
  font-weight: 600;
  display: block;
  margin-bottom: 2px;
}

.warning-text p {
  margin: 0;
  line-height: 1.5;
  color: #a16207;
}

.warning-dismiss {
  border: 1px solid #fde68a;
  background: #ffffff;
  color: #92400e;
  font-size: 11px;
  font-weight: 600;
  padding: 4px 12px;
  border-radius: 6px;
  cursor: pointer;
  white-space: nowrap;
  font-family: inherit;
  flex-shrink: 0;
}

.warning-dismiss:hover {
  background: #fef3c7;
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
