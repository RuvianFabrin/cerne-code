<script setup lang="ts">
import { computed, nextTick, watch, ref, onMounted, onUnmounted } from "vue";
import { useSessionStore } from "../stores/session";
import MessageBubble from "./MessageBubble.vue";
import MarkdownContent from "./MarkdownContent.vue";
import ComposerBar from "./ComposerBar.vue";
import DiffReview from "./DiffReview.vue";
import AskCard from "./AskCard.vue";
import PermissionCard from "./PermissionCard.vue";
import TaskStepGroup from "./TaskStepGroup.vue";
import TodoCard from "./TodoCard.vue";
import { formatElapsed, formatTokens } from "../taskLabels";
import type { ChatMessage, TaskItem, TurnStats } from "../api";

defineEmits<{ "open-settings": [] }>();

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
  | { kind: "stats"; key: string; stats: TurnStats };

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
    }
  });
  if (userTurn > 0 && stats[userTurn]) {
    items.push({ kind: "stats", key: `stats-${userTurn}`, stats: stats[userTurn] });
  }
  return items;
});

const statusLabel = computed(() => {
  if (sessionStore.status === "starting_server") return "Iniciando servidor local...";
  if (sessionStore.status === "thinking") {
    // Nem todo provider/modelo transmite tokens de raciocínio visíveis
    // (thinkingText) — sem isso, "Pensando..." parado por minutos parece
    // travado mesmo não estando. Enquanto não chegou nenhum token de
    // raciocínio nem de resposta, deixa claro que é o modelo processando
    // (TTFT), não uma sessão de "pensamento" visível.
    const hasVisibleReasoning = !!sessionStore.thinkingText;
    const base = hasVisibleReasoning ? "Pensando..." : "Aguardando resposta do modelo...";
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
    <p>Crie uma nova sessão pra começar.</p>
  </div>
  <template v-else>
    <div class="chat-layout">
      <div class="chat-column">
        <div class="chat-scroll" ref="scrollRef" @scroll="onChatScroll">
          <div class="chat-inner">
            <template v-for="item in timeline" :key="item.key">
              <MessageBubble v-if="item.kind === 'message'" :message="item.message" />
              <TaskStepGroup v-else-if="item.kind === 'steps'" :tasks="item.tasks" />
              <TodoCard v-else-if="item.kind === 'todo'" :todos="item.todos" />
              <div v-else-if="item.kind === 'stats'" class="turn-stats">
                <span class="msi">schedule</span>
                {{ formatElapsed(item.stats.elapsed_ms) }}
                <span class="stats-sep">·</span>
                {{ formatTokens(item.stats.prompt_tokens + item.stats.completion_tokens) }} tokens
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
                <strong>Automação de tela ativa</strong>
                <p>Cada screenshot consome ~1.500 tokens de visão. Uma sessão com 20 ações pode usar ~30K tokens extras.</p>
              </div>
              <button class="warning-dismiss" @click="sessionStore.showComputerUseWarning = false">Entendi</button>
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
        <div class="composer-wrap">
          <button v-if="showJumpToBottom" class="jump-to-bottom" @click="jumpToBottom" title="Ir para o final">
            <span class="msi">arrow_downward</span>
          </button>
          <!-- Tudo que pede uma ação do usuário (aceitar edição, aprovar
               ferramenta em modo Manual, responder pergunta) fica ancorado
               aqui, fora da área de scroll — enterrado lá no topo, junto
               com o histórico, passava despercebido. -->
          <div v-if="sessionStore.pendingEdits.length > 0" class="pending-edits-anchor">
            <DiffReview v-for="edit in sessionStore.pendingEdits" :key="edit.id" :edit="edit" />
          </div>
          <PermissionCard />
          <AskCard />
          <ComposerBar />
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
}

.chat-column {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
}

.chat-scroll {
  flex: 1;
  overflow-y: auto;
  scroll-padding-bottom: 24px;
}

.chat-inner {
  max-width: 820px;
  margin: 0 auto;
  padding: 24px 24px 24px;
}

.composer-wrap {
  position: relative;
  max-width: 820px;
  width: 100%;
  margin: 0 auto;
  padding: 0 24px 20px;
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
