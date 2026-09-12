<script setup lang="ts">
import { ref, watch } from "vue";
import Dialog from "primevue/dialog";
import { api } from "../api";

// Fase 6, Degrau 1 do roteiro (skill store — sem registry/hospedagem nova):
// importa uma skill de qualquer URL que sirva markdown cru (raw de gist/
// GitHub/qualquer host). SEMPRE mostra o conteúdo completo num preview antes
// de salvar — skill é texto lido pelo LLM, o risco real de importar de
// terceiro é prompt injection, não "permissão"; nunca instala direto sem o
// usuário ver o texto primeiro.
const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ "update:visible": [value: boolean]; saved: [] }>();

const url = ref("");
const content = ref("");
const fetching = ref(false);
const saving = ref(false);
const error = ref("");

watch(
  () => props.visible,
  (visible) => {
    if (!visible) return;
    url.value = "";
    content.value = "";
    error.value = "";
  },
);

async function fetchPreview() {
  if (!url.value.trim()) return;
  error.value = "";
  content.value = "";
  fetching.value = true;
  try {
    content.value = await api.fetchSkillFromUrl(url.value.trim());
  } catch (e) {
    error.value = String(e);
  } finally {
    fetching.value = false;
  }
}

async function confirmImport() {
  error.value = "";
  saving.value = true;
  try {
    await api.importSkill(content.value);
    emit("saved");
    emit("update:visible", false);
  } catch (e) {
    error.value = String(e);
  } finally {
    saving.value = false;
  }
}

function cancel() {
  emit("update:visible", false);
}
</script>

<template>
  <Dialog
    :visible="props.visible"
    @update:visible="(v) => emit('update:visible', v)"
    :header="$t('skillImport.header')"
    modal
    :style="{ width: '640px' }"
  >
    <div class="skill-import-help">
      <p>{{ $t("skillImport.whatIsThis") }}</p>
    </div>
    <div class="skill-import-url-row">
      <input
        v-model="url"
        class="text-input"
        :placeholder="$t('skillImport.urlPlaceholder')"
        @keydown.enter="fetchPreview"
      />
      <button class="btn-secondary" :disabled="fetching || !url.trim()" @click="fetchPreview">
        {{ fetching ? $t("skillImport.fetching") : $t("skillImport.fetchButton") }}
      </button>
    </div>
    <template v-if="content">
      <p class="skill-import-preview-label">{{ $t("skillImport.previewLabel") }}</p>
      <textarea v-model="content" class="skill-import-textarea" rows="14" spellcheck="false" />
    </template>
    <p v-if="error" class="error-text">{{ error }}</p>
    <template #footer>
      <button class="btn-secondary" @click="cancel">{{ $t("newSession.cancel") }}</button>
      <button class="btn-primary" :disabled="!content || saving" @click="confirmImport">
        {{ saving ? $t("skillImport.saving") : $t("skillImport.importButton") }}
      </button>
    </template>
  </Dialog>
</template>

<style scoped>
.skill-import-help {
  background: #f4f4f5;
  border-radius: 8px;
  padding: 10px 12px;
  margin-bottom: 12px;
}

.skill-import-help p {
  margin: 0;
  font-size: 12px;
  font-weight: 500;
  color: #3f3f46;
  line-height: 1.5;
}

.skill-import-url-row {
  display: flex;
  gap: 8px;
  margin-bottom: 10px;
}

.text-input {
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  outline: none;
  flex: 1;
  box-sizing: border-box;
}

.skill-import-preview-label {
  margin: 0 0 6px;
  font-size: 11px;
  font-weight: 600;
  color: #71717a;
}

.skill-import-textarea {
  width: 100%;
  box-sizing: border-box;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 10px 12px;
  font-size: 12px;
  font-family: var(--cerne-mono);
  resize: vertical;
  outline: none;
}

.error-text {
  font-size: 12px;
  font-weight: 500;
  color: #dc2626;
  margin-top: 8px;
}

.btn-primary {
  border: none;
  background: #18181b;
  color: #ffffff;
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.btn-primary:disabled {
  background: #e4e4e7;
  color: #a1a1aa;
  cursor: default;
}

.btn-secondary {
  border: var(--cerne-border);
  background: #ffffff;
  color: #52525b;
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
  margin-right: 8px;
}

.btn-secondary:disabled {
  opacity: 0.5;
  cursor: default;
}
</style>
