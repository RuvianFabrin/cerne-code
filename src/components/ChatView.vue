<script setup lang="ts">
import { computed, nextTick, watch, ref } from "vue";
import { useSessionStore } from "../stores/session";
import MessageBubble from "./MessageBubble.vue";
import MarkdownContent from "./MarkdownContent.vue";
import ComposerBar from "./ComposerBar.vue";
import DiffReview from "./DiffReview.vue";
import AskCard from "./AskCard.vue";
import PermissionCard from "./PermissionCard.vue";
import TaskStepGroup from "./TaskStepGroup.vue";
import TodoCard from "./TodoCard.vue";
import type { ChatMessage, TaskItem } from "../api";

defineEmits<{ "open-settings": [] }>();

const sessionStore = useSessionStore();
const scrollRef = ref<HTMLDivElement | null>(null);

// Timeline do chat: mensagem do usuário -> passos de ferramenta daquele
// turno (rótulo amigável, expansível) -> resposta final do agente. As
// mensagens de assistant sem conteúdo (só tool_calls, sem texto) não viram
// bolha vazia — o passo já aparece representado no grupo de steps.
type TimelineItem =
  | { kind: "message"; key: string; message: ChatMessage }
  | { kind: "steps"; key: string; tasks: TaskItem[] }
  | { kind: "todo"; key: string; todos: import("../api").TodoItem[] };

const timeline = computed<TimelineItem[]>(() => {
  const items: TimelineItem[] = [];
  let userTurn = 0;
  let todoIdx = 0;
  let taskOffset = 0;
  const allTasks = sessionStore.tasks;
  sessionStore.messages.forEach((m, i) => {
    if (m.role === "user") {
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
  return items;
});

const statusLabel = computed(() => {
  if (sessionStore.status === "starting_server") return "Iniciando servidor local...";
  if (sessionStore.status === "thinking") return "Pensando...";
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
  () => [sessionStore.messages.length, sessionStore.streamingText, sessionStore.thinkingText],
  async () => {
    await nextTick();
    scrollRef.value?.scrollTo({ top: scrollRef.value.scrollHeight });
  },
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
        <div class="chat-scroll" ref="scrollRef">
          <div class="chat-inner">
            <DiffReview v-for="edit in sessionStore.pendingEdits" :key="edit.id" :edit="edit" />
            <template v-for="item in timeline" :key="item.key">
              <MessageBubble v-if="item.kind === 'message'" :message="item.message" />
              <TaskStepGroup v-else-if="item.kind === 'steps'" :tasks="item.tasks" />
              <TodoCard v-else :todos="item.todos" />
            </template>
            <AskCard />
            <PermissionCard />
            <div v-if="sessionStore.streamingText" class="row">
              <div class="bubble streaming">
                <MarkdownContent :content="sessionStore.streamingText" />
              </div>
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
  max-width: 820px;
  width: 100%;
  margin: 0 auto;
  padding: 0 24px 20px;
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

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
