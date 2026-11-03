<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import { api } from "../api";
import { useSessionStore } from "../stores/session";

// Modal da fila de tarefas SEM Long Horizon: memória da sessão (vai inteira ao
// modelo, nunca compactada) e a lista de tarefas com Iniciar/Parar.
const visible = defineModel<boolean>("visible", { required: true });
const { t } = useI18n();
const sessionStore = useSessionStore();

const tab = ref<"memoria" | "fila">("memoria");
const memoria = ref("");
const fila = ref("");
const saveError = ref("");
const saved = ref(false);
let savedTimer: ReturnType<typeof setTimeout> | undefined;

watch(visible, async (open) => {
  const id = sessionStore.currentId;
  if (!open || !id) return;
  tab.value = "memoria";
  saveError.value = "";
  try {
    [memoria.value, fila.value] = await Promise.all([api.readSessionMemory(id), api.readSessionPlainQueue(id)]);
  } catch (e) {
    saveError.value = String(e);
  }
});

const running = computed(() => !!sessionStore.currentId && sessionStore.plainQueueRunningIds.has(sessionStore.currentId));
// Mesma exigência da fila do LH: sem YOLO a fila travaria em cada permissão de ferramenta.
const canStart = computed(() => sessionStore.currentSession?.execution_mode === "yolo");

function flashSaved() {
  saved.value = true;
  clearTimeout(savedTimer);
  savedTimer = setTimeout(() => (saved.value = false), 2000);
}

async function saveMemoria() {
  if (!sessionStore.currentId) return;
  try {
    await api.writeSessionMemory(sessionStore.currentId, memoria.value);
    saveError.value = "";
    flashSaved();
  } catch (e) {
    saveError.value = String(e);
  }
}

async function saveFila() {
  if (!sessionStore.currentId) return;
  try {
    await api.writeSessionPlainQueue(sessionStore.currentId, fila.value);
    saveError.value = "";
    flashSaved();
  } catch (e) {
    saveError.value = String(e);
  }
}

async function toggleQueue() {
  if (running.value) {
    await sessionStore.stopPlainQueue();
    return;
  }
  if (!canStart.value) return;
  // Salva o que está no campo antes de iniciar — o loop lê do disco.
  await saveFila();
  if (saveError.value) return;
  try {
    await sessionStore.startPlainQueue();
  } catch (e) {
    saveError.value = String(e);
  }
}
</script>

<template>
  <Dialog
    v-model:visible="visible"
    modal
    :header="t('plainQueue.title')"
    :style="{ width: 'min(760px, 94vw)' }"
    class="plain-queue-dialog"
  >
    <div class="plain-queue">
      <div class="plain-queue-tabs" role="tablist">
        <button type="button" role="tab" :class="{ active: tab === 'memoria' }" @click="tab = 'memoria'">
          {{ t("plainQueue.memoriaTab") }}
        </button>
        <button type="button" role="tab" :class="{ active: tab === 'fila' }" @click="tab = 'fila'">
          {{ t("plainQueue.filaTab") }}
        </button>
      </div>

      <template v-if="tab === 'memoria'">
        <p class="hint">{{ t("plainQueue.memoriaHint") }}</p>
        <textarea
          v-model="memoria"
          class="text-input plain-queue-textarea"
          :placeholder="t('plainQueue.memoriaPlaceholder')"
        />
        <div class="plain-queue-actions">
          <button class="btn-primary" @click="saveMemoria">{{ t("sidebar.save") }}</button>
        </div>
      </template>

      <template v-else>
        <p class="hint">{{ t("plainQueue.filaHint") }}</p>
        <code class="plain-queue-format">{{ '{"tarefas": [{"id": 1, "descricao": "...", "status": "fazer"}]}' }}</code>
        <textarea
          v-model="fila"
          class="text-input plain-queue-textarea"
          :placeholder="t('plainQueue.filaPlaceholder')"
        />
        <p v-if="!canStart && !running" class="hint">{{ t("plainQueue.requirements") }}</p>
        <div class="plain-queue-actions">
          <button class="btn-secondary" @click="saveFila">{{ t("sidebar.save") }}</button>
          <button class="btn-primary" :disabled="!running && !canStart" @click="toggleQueue">
            <span class="msi">{{ running ? "stop" : "play_arrow" }}</span>
            {{ running ? t("plainQueue.stop") : t("plainQueue.start") }}
          </button>
          <span v-if="sessionStore.plainQueueLastEvent" class="plain-queue-event" :class="sessionStore.plainQueueLastEvent.status">
            {{ sessionStore.plainQueueLastEvent.message ?? t(`plainQueue.status.${sessionStore.plainQueueLastEvent.status}`) }}
          </span>
        </div>
      </template>

      <p v-if="saveError" class="error-text">{{ saveError }}</p>
      <span v-if="saved" class="mcp-test-success" role="status">{{ t("composer.longHorizonSaved") }}</span>
    </div>
  </Dialog>
</template>

<style scoped>
.plain-queue {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.plain-queue-tabs {
  display: flex;
  gap: 6px;
}
.plain-queue-tabs button {
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 6px 12px;
  background: transparent;
  font: inherit;
  font-size: 13px;
  cursor: pointer;
}
.plain-queue-tabs button.active {
  font-weight: 600;
  border-color: currentColor;
}
.plain-queue-textarea {
  min-height: 280px;
  font-family: "JetBrains Mono", monospace;
  font-size: 13px;
  resize: vertical;
}
.plain-queue-format {
  font-size: 12px;
  opacity: 0.8;
  overflow-x: auto;
  white-space: nowrap;
}
.plain-queue-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 8px;
}
.plain-queue-event {
  font-size: 13px;
}
.plain-queue-event.stuck,
.plain-queue-event.error {
  color: #b45309;
}
</style>
