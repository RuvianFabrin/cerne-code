<script setup lang="ts">
import { ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Dialog from "primevue/dialog";
import { useProviderStore } from "../stores/provider";
import { useSessionStore } from "../stores/session";
import ProviderPicker from "./ProviderPicker.vue";
import type { ProviderKind } from "../api";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();

const providerStore = useProviderStore();
const sessionStore = useSessionStore();

const title = ref("Nova sessão");
const projectRoot = ref<string | null>(null);
// Own picker state, seeded from the global defaults but independent from
// them from here on — this dialog is the only place those defaults are
// actually used (existing sessions never re-read them, see ComposerBar).
const provider = ref<ProviderKind>("ollama");
const fork = ref("turboquant");
const customProviderId = ref("");
const model = ref<string | null>(null);

watch(
  () => props.visible,
  (v) => {
    if (v) {
      title.value = "Nova sessão";
      projectRoot.value = null;
      provider.value = providerStore.config?.active_provider ?? "ollama";
      fork.value = providerStore.config?.active_llama_fork ?? "turboquant";
      customProviderId.value = providerStore.config?.active_custom_provider_id ?? "";
      model.value = providerStore.config?.active_model ?? null;
    }
  },
);

async function pickFolder() {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected === "string") projectRoot.value = selected;
}

async function create() {
  if (!model.value) return;
  await sessionStore.createSession(
    title.value.trim() || "Nova sessão",
    provider.value,
    model.value,
    projectRoot.value,
    provider.value === "llama_cpp" ? fork.value : null,
    provider.value === "custom" ? customProviderId.value : null,
  );
  emit("update:visible", false);
}
</script>

<template>
  <Dialog
    :visible="props.visible"
    @update:visible="(v) => emit('update:visible', v)"
    header="Nova sessão"
    modal
    :style="{ width: '440px' }"
  >
    <div class="field">
      <label>Título</label>
      <input v-model="title" class="text-input" placeholder="Ex: refatorar API de sessões" />
    </div>
    <div class="field">
      <label>Pasta do projeto (opcional — sem ela o agente só tem busca na web e MCP, sem leitura/edição de arquivos nem execução de comandos)</label>
      <button class="folder-btn" @click="pickFolder">
        <span class="msi">folder_open</span>
        <span class="folder-path">{{ projectRoot ?? "Escolher pasta..." }}</span>
      </button>
    </div>
    <div class="field">
      <label>Provider e modelo</label>
      <ProviderPicker
        v-model:provider="provider"
        v-model:fork="fork"
        v-model:custom-provider-id="customProviderId"
        v-model:model="model"
      />
      <p v-if="providerStore.error" class="error-text">{{ providerStore.error }}</p>
    </div>
    <template #footer>
      <button class="btn-secondary" @click="emit('update:visible', false)">Cancelar</button>
      <button class="btn-primary" :disabled="!model" @click="create">Criar sessão</button>
    </template>
  </Dialog>
</template>

<style scoped>
.field {
  margin-bottom: 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

label {
  font-size: 12px;
  font-weight: 600;
  color: #52525b;
}

.error-text {
  font-size: 12px;
  font-weight: 500;
  color: #dc2626;
  margin: 6px 0 0;
}

.text-input {
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  outline: none;
}

.folder-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
  background: #ffffff;
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  color: #3f3f46;
  text-align: left;
}

.folder-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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
</style>
