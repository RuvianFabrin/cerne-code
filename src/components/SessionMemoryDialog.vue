<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import { api } from "../api";
import { useSessionStore } from "../stores/session";

// Memória da sessão: botão fixo no composer, independente da fila de tarefas.
// Vai inteira ao modelo em todo turno e nunca é compactada (ver agent/plain_queue.rs).
const visible = defineModel<boolean>("visible", { required: true });
const { t } = useI18n();
const sessionStore = useSessionStore();

const memoria = ref("");
const saveError = ref("");
const saved = ref(false);
let savedTimer: ReturnType<typeof setTimeout> | undefined;

watch(visible, async (open) => {
  const id = sessionStore.currentId;
  if (!open || !id) return;
  saveError.value = "";
  try {
    memoria.value = await api.readSessionMemory(id);
  } catch (e) {
    saveError.value = String(e);
  }
});

async function saveMemoria() {
  if (!sessionStore.currentId) return;
  try {
    await api.writeSessionMemory(sessionStore.currentId, memoria.value);
    saveError.value = "";
    saved.value = true;
    clearTimeout(savedTimer);
    savedTimer = setTimeout(() => (saved.value = false), 2000);
  } catch (e) {
    saveError.value = String(e);
  }
}
</script>

<template>
  <Dialog
    v-model:visible="visible"
    modal
    :header="t('plainQueue.memoryTitle')"
    :style="{ width: 'min(680px, 94vw)' }"
    class="mem-dialog"
  >
    <div class="mem">
      <p class="mem-hint">{{ t("plainQueue.memoryHint") }}</p>
      <textarea
        v-model="memoria"
        class="mem-textarea"
        :placeholder="t('plainQueue.memoryPlaceholder')"
      />
      <p v-if="saveError" class="mem-error">{{ saveError }}</p>
    </div>

    <footer class="mem-footer">
      <button type="button" class="mem-btn" @click="saveMemoria">
        <span class="msi">save</span>
        {{ t("sidebar.save") }}
      </button>
      <span v-if="saved" class="mem-saved">
        <span class="msi">check_circle</span>
        {{ t("composer.longHorizonSaved") }}
      </span>
    </footer>
  </Dialog>
</template>

<style scoped>
.mem {
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding-top: 4px;
}

.mem-hint {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
  margin: 0;
  line-height: 1.5;
}

.mem-textarea {
  min-height: 340px;
  width: 100%;
  box-sizing: border-box;
  border: 1px solid #d4d4d8;
  border-radius: 8px;
  padding: 10px 12px;
  font-family: var(--cerne-mono, monospace);
  font-size: 13px;
  line-height: 1.6;
  color: #18181b;
  background: #ffffff;
  resize: vertical;
  outline: none;
}

.mem-textarea:focus {
  border-color: #18181b;
  box-shadow: 0 0 0 3px rgba(24, 24, 27, 0.08);
}

.mem-error {
  font-size: 12px;
  font-weight: 500;
  color: #dc2626;
  margin: 0;
}

.mem-footer {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-top: 16px;
  padding-top: 14px;
  border-top: 1px solid #e4e4e7;
}

.mem-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  min-height: 34px;
  border: none;
  border-radius: 8px;
  padding: 6px 14px;
  background: #18181b;
  color: #ffffff;
  font-family: inherit;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.mem-btn:hover {
  background: #3f3f46;
}

.mem-btn .msi {
  font-size: 18px;
}

.mem-saved {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
  color: #16a34a;
}

.mem-saved .msi {
  font-size: 15px;
}
</style>
