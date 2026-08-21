<script setup lang="ts">
import { ref, watch } from "vue";
import Dialog from "primevue/dialog";
import { api, type DirEntryInfo } from "../api";
import { useSessionStore } from "../stores/session";
import FileTreeNode from "./FileTreeNode.vue";
import FolderPicker from "./FolderPicker.vue";

// Fase D2 do roteiro de Agentes/Skills: navegador de arquivos da pasta
// escolhida — clicar num arquivo (ou no ícone de "inserir" de uma pasta)
// insere o caminho absoluto no composer.

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();

const sessionStore = useSessionStore();
const rootPath = ref("");
const rootEntries = ref<DirEntryInfo[]>([]);
const loading = ref(false);
const error = ref("");

async function loadRoot() {
  if (!rootPath.value) return;
  loading.value = true;
  error.value = "";
  try {
    rootEntries.value = await api.listDirEntries(rootPath.value);
  } catch (e) {
    error.value = String(e);
    rootEntries.value = [];
  } finally {
    loading.value = false;
  }
}

watch(
  () => props.visible,
  (v) => {
    if (!v) return;
    rootPath.value =
      sessionStore.currentSession?.project_root ??
      sessionStore.currentSession?.extra_read_paths?.[0]?.path ??
      "";
    if (rootPath.value) loadRoot();
  },
);

async function onRootPick(path: string) {
  rootPath.value = path;
  await loadRoot();
}

function onPick(path: string) {
  sessionStore.setDraft(path);
  emit("update:visible", false);
}
</script>

<template>
  <Dialog
    :visible="visible"
    @update:visible="(v) => emit('update:visible', v)"
    :header="$t('fileBrowser.title')"
    modal
    :style="{ width: '520px' }"
  >
    <div class="fb-root-row">
      <FolderPicker :current="rootPath" @pick="onRootPick" />
    </div>
    <div class="fb-tree">
      <p v-if="loading" class="hint">{{ $t("fileBrowser.loading") }}</p>
      <p v-else-if="error" class="hint fb-error">{{ error }}</p>
      <p v-else-if="!rootPath" class="hint">{{ $t("fileBrowser.pickHint") }}</p>
      <p v-else-if="rootEntries.length === 0" class="hint">{{ $t("fileBrowser.empty") }}</p>
      <FileTreeNode v-for="entry in rootEntries" :key="entry.path" :entry="entry" :depth="0" @pick="onPick" />
    </div>
  </Dialog>
</template>

<style scoped>
.fb-root-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}

.fb-tree {
  max-height: 420px;
  overflow-y: auto;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 6px;
}

.hint {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
  padding: 4px;
}

.fb-error {
  color: #dc2626;
}
</style>
