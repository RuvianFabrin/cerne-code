<script setup lang="ts">
import { ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import { api, type PythonTool } from "../api";

// T17: editor de uma ferramenta Python que o LLM criou (`create_python_tool`)
// em algum turno anterior. Diferente de Skill/Persona, não existe botão de
// "criar nova" aqui de propósito — a criação é sempre pelo LLM, o usuário só
// vê/edita/apaga o que já foi gerado (mesma decisão de escopo do T17).
const { t } = useI18n();
const props = defineProps<{ visible: boolean; tool: PythonTool | null }>();
const emit = defineEmits<{ "update:visible": [value: boolean]; saved: [] }>();

const description = ref("");
const script = ref("");
const dependenciesText = ref("");
const error = ref("");

watch(
  () => [props.visible, props.tool?.name] as const,
  ([visible]) => {
    if (!visible) return;
    error.value = "";
    description.value = props.tool?.description ?? "";
    script.value = props.tool?.script ?? "";
    dependenciesText.value = (props.tool?.dependencies ?? []).join(", ");
  },
);

async function save() {
  error.value = "";
  if (!props.tool) return;
  if (!description.value.trim() || !script.value.trim()) {
    error.value = t("pythonToolEditor.fillDescriptionAndScript");
    return;
  }
  const dependencies = dependenciesText.value
    .split(",")
    .map((d) => d.trim())
    .filter((d) => d.length > 0);
  try {
    await api.updatePythonTool(props.tool.name, description.value.trim(), script.value, dependencies);
    emit("saved");
    emit("update:visible", false);
  } catch (e) {
    error.value = String(e);
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
    :header="$t('pythonToolEditor.header', { name: tool?.name ?? '' })"
    modal
    :style="{ width: '680px' }"
  >
    <div class="pyt-help">
      <p>{{ $t("pythonToolEditor.whatIsThis") }}</p>
      <p v-if="tool" class="pyt-path">{{ tool.tool_path }}</p>
    </div>
    <label class="pyt-label">{{ $t("pythonToolEditor.descriptionLabel") }}</label>
    <input v-model="description" class="text-input" :placeholder="$t('pythonToolEditor.descriptionPlaceholder')" />
    <label class="pyt-label">{{ $t("pythonToolEditor.dependenciesLabel") }}</label>
    <input v-model="dependenciesText" class="text-input" :placeholder="$t('pythonToolEditor.dependenciesPlaceholder')" />
    <label class="pyt-label">{{ $t("pythonToolEditor.scriptLabel") }}</label>
    <textarea v-model="script" class="pyt-textarea" rows="14" spellcheck="false" />
    <p v-if="error" class="error-text">{{ error }}</p>
    <template #footer>
      <button class="btn-secondary" @click="cancel">{{ $t("newSession.cancel") }}</button>
      <button class="btn-primary" @click="save">{{ $t("sidebar.save") }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.pyt-help {
  background: #f4f4f5;
  border-radius: 8px;
  padding: 10px 12px;
  margin-bottom: 12px;
}

.pyt-help p {
  margin: 0;
  font-size: 12px;
  font-weight: 500;
  color: #3f3f46;
  line-height: 1.5;
}

.pyt-path {
  margin-top: 4px !important;
  font-family: var(--cerne-mono);
  font-size: 11px !important;
  color: #71717a !important;
  word-break: break-all;
}

.pyt-label {
  display: block;
  font-size: 11px;
  font-weight: 600;
  color: #71717a;
  margin: 10px 0 4px;
}

.text-input {
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  outline: none;
  width: 100%;
  box-sizing: border-box;
}

.pyt-textarea {
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
</style>
