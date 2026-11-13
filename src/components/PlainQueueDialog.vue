<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import { open } from "@tauri-apps/plugin-dialog";
import { useSessionStore } from "../stores/session";

// Modal da fila de tarefas SEM Long Horizon: escolhe o arquivo JSON da lista
// e inicia/para a fila. A memória da sessão tem modal próprio (SessionMemoryDialog).
const visible = defineModel<boolean>("visible", { required: true });
const { t } = useI18n();
const sessionStore = useSessionStore();

const caminho = ref("");
const saveError = ref("");
const saved = ref(false);
let savedTimer: ReturnType<typeof setTimeout> | undefined;

watch(visible, (isOpen) => {
  if (!isOpen) return;
  saveError.value = "";
  caminho.value = sessionStore.currentSession?.plain_queue_path ?? "";
});

// Modo Anything: a fila lê sozinha o tarefas.json do alvo planejado; não há arquivo a escolher.
const anything = computed(() => !!sessionStore.currentSession?.anything?.enabled);
const running = computed(() => !!sessionStore.currentId && sessionStore.plainQueueRunningIds.has(sessionStore.currentId));
// Mesma exigência da fila do LH: sem YOLO a fila travaria em cada permissão de ferramenta.
const canStart = computed(() => sessionStore.currentSession?.execution_mode === "yolo");

function flashSaved() {
  saved.value = true;
  clearTimeout(savedTimer);
  savedTimer = setTimeout(() => (saved.value = false), 2000);
}

async function browse() {
  const selected = await open({
    directory: false,
    multiple: false,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (typeof selected === "string") caminho.value = selected;
}

async function savePath(): Promise<boolean> {
  if (!sessionStore.currentId) return false;
  try {
    await sessionStore.updatePlainQueuePath(caminho.value.trim() || null);
    saveError.value = "";
    flashSaved();
    return true;
  } catch (e) {
    saveError.value = String(e);
    return false;
  }
}

async function toggleQueue() {
  if (running.value) {
    await sessionStore.stopPlainQueue();
    return;
  }
  if (!canStart.value) return;
  if (!anything.value) {
    if (!caminho.value.trim()) {
      saveError.value = t("plainQueue.pathMissing");
      return;
    }
    // Grava o caminho antes de iniciar — o loop lê o arquivo a partir da sessão.
    if (!(await savePath())) return;
  }
  saveError.value = "";
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
    :style="{ width: 'min(620px, 94vw)' }"
    class="pq-dialog"
  >
    <div class="pq">
      <p v-if="anything" class="pq-hint">{{ t("plainQueue.anythingAuto") }}</p>
      <template v-else>
      <label class="pq-label" for="pq-path">{{ t("plainQueue.pathLabel") }}</label>
      <div class="pq-path">
        <span class="msi pq-path-icon">description</span>
        <input
          id="pq-path"
          v-model="caminho"
          class="pq-input"
          :placeholder="t('plainQueue.pathPlaceholder')"
          spellcheck="false"
          @keydown.enter.prevent="savePath"
        />
        <button type="button" class="pq-btn pq-btn-ghost" @click="browse">
          <span class="msi">folder_open</span>
          {{ t("plainQueue.pathBrowse") }}
        </button>
      </div>
      <p class="pq-hint">{{ t("plainQueue.pathHint") }}</p>
      <code class="pq-format">{{ '{"tarefas": [{"id": 1, "descricao": "...", "status": "fazer"}]}' }}</code>
      </template>
      <p v-if="!canStart && !running" class="pq-warn">
        <span class="msi">warning</span>
        {{ t("plainQueue.requirements") }}
      </p>
      <p v-if="saveError" class="pq-error">{{ saveError }}</p>
    </div>

    <footer class="pq-footer">
      <button v-if="!anything" type="button" class="pq-btn pq-btn-ghost" @click="savePath">
        <span class="msi">save</span>
        {{ t("sidebar.save") }}
      </button>
      <button
        type="button"
        class="pq-btn pq-btn-primary"
        :class="{ 'pq-btn-stop': running }"
        :disabled="!running && !canStart"
        @click="toggleQueue"
      >
        <span class="msi">{{ running ? "stop" : "play_arrow" }}</span>
        {{ running ? t("plainQueue.stop") : t("plainQueue.start") }}
      </button>
      <span v-if="saved" class="pq-saved">
        <span class="msi">check_circle</span>
        {{ t("composer.longHorizonSaved") }}
      </span>
      <span
        v-if="sessionStore.plainQueueLastEvent"
        class="pq-event"
        :class="sessionStore.plainQueueLastEvent.status"
      >
        {{ sessionStore.plainQueueLastEvent.message ?? t(`plainQueue.status.${sessionStore.plainQueueLastEvent.status}`) }}
      </span>
    </footer>
  </Dialog>
</template>

<style scoped>
.pq {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-top: 4px;
}

.pq-label {
  font-size: 13px;
  font-weight: 600;
  color: #18181b;
}

.pq-path {
  display: flex;
  align-items: center;
  gap: 8px;
}

.pq-path-icon {
  color: #a1a1aa;
  font-size: 20px;
}

.pq-input {
  flex: 1;
  min-width: 0;
  border: 1px solid #d4d4d8;
  border-radius: 8px;
  padding: 8px 10px;
  font-family: var(--cerne-mono, monospace);
  font-size: 13px;
  color: #18181b;
  background: #ffffff;
  outline: none;
}

.pq-input:focus {
  border-color: #18181b;
  box-shadow: 0 0 0 3px rgba(24, 24, 27, 0.08);
}

.pq-hint {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
  margin: 2px 0 0;
  line-height: 1.5;
}

.pq-format {
  display: block;
  font-family: var(--cerne-mono, monospace);
  font-size: 11px;
  background: rgba(0, 0, 0, 0.04);
  padding: 6px 8px;
  border-radius: 6px;
  word-break: break-all;
}

.pq-warn,
.pq-error {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
  margin: 0;
}

.pq-warn {
  color: #b45309;
}

.pq-error {
  color: #dc2626;
}

.pq-footer {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 10px;
  margin-top: 16px;
  padding-top: 14px;
  border-top: 1px solid #e4e4e7;
}

.pq-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  min-height: 34px;
  border-radius: 8px;
  padding: 6px 14px;
  font-family: inherit;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s, opacity 0.15s;
}

.pq-btn:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}

.pq-btn .msi {
  font-size: 18px;
}

.pq-btn-ghost {
  border: 1px solid #d4d4d8;
  background: #ffffff;
  color: #18181b;
}

.pq-btn-ghost:hover:not(:disabled) {
  background: #f4f4f5;
}

.pq-btn-primary {
  border: none;
  background: #18181b;
  color: #ffffff;
}

.pq-btn-primary:hover:not(:disabled) {
  background: #3f3f46;
}

.pq-btn-stop {
  background: #dc2626;
}

.pq-btn-stop:hover:not(:disabled) {
  background: #b91c1c;
}

.pq-saved {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
  color: #16a34a;
}

.pq-saved .msi {
  font-size: 15px;
}

.pq-event {
  margin-left: auto;
  font-size: 12px;
  font-weight: 500;
  color: #52525b;
}

.pq-event.stuck,
.pq-event.error {
  color: #dc2626;
}

.pq-event.finished,
.pq-event.confirmed {
  color: #16a34a;
}
</style>
