<script setup lang="ts">
import { computed, ref, watch } from "vue";
import Dialog from "primevue/dialog";
import { classifyDiffLines } from "../diffUtils";
import { useSessionStore } from "../stores/session";
import FolderPicker from "./FolderPicker.vue";
import { api, type GitFileChange } from "../api";

// Fase D1 do roteiro de Agentes/Skills, revisitada em 2026-08-17: antes
// agregava diffs a partir do histórico de tool calls da sessão (perdia
// edição manual do usuário, não distinguia arquivo novo/deletado/renomeado
// direito) — pedido do usuário testando ao vivo pra pegar o diff de
// verdade do `git` (`git.rs` no backend), igual o painel "Controle do
// Código-Fonte" do VS Code. Só funciona se `project_root` for um
// repositório git — fora disso mostra um aviso em vez da lista.

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();

const sessionStore = useSessionStore();

const projectRoot = computed(() => sessionStore.currentSession?.project_root ?? "");

const changes = ref<GitFileChange[]>([]);
const loadingStatus = ref(false);
const statusError = ref<string | null>(null);

async function loadStatus() {
  if (!projectRoot.value) {
    changes.value = [];
    statusError.value = null;
    return;
  }
  loadingStatus.value = true;
  statusError.value = null;
  try {
    changes.value = await api.gitRepoStatus(projectRoot.value);
  } catch (e) {
    changes.value = [];
    statusError.value = String(e);
  } finally {
    loadingStatus.value = false;
  }
}

watch(() => props.visible, (v) => { if (v) loadStatus(); });
// Só recarrega no troca de pasta se o modal já estiver aberto — sem essa
// guarda, `git status`/`git diff` rodavam toda vez que o usuário trocava de
// SESSÃO (o `project_root` muda junto), mesmo com o modal fechado, sem
// nenhum motivo (achado testando ao vivo, 2026-08-17).
watch(projectRoot, () => { if (props.visible) loadStatus(); });

// Um arquivo pode estar modificado no git E ter uma edição sandboxed
// pendente de aceitar (modo Manual/Auto) ao mesmo tempo — badge extra pra
// deixar isso claro. `target_path` de `PendingEdit` é absoluto; normaliza
// separador pra comparar com o caminho relativo que o git devolve.
function isPending(relPath: string): boolean {
  const normalizedRel = relPath.replace(/\\/g, "/");
  return sessionStore.pendingEdits.some((e) => e.target_path.replace(/\\/g, "/").endsWith(normalizedRel));
}

const STATUS_LABELS: Record<GitFileChange["status"], string> = {
  modified: "M",
  added: "A",
  deleted: "D",
  renamed: "R",
  untracked: "U",
};

const selectedPath = ref<string | null>(null);
const diffCache = ref<Map<string, string>>(new Map());
const loadingDiff = ref(false);
const diffError = ref<string | null>(null);

async function selectFile(path: string) {
  selectedPath.value = path;
  if (diffCache.value.has(path)) return;
  loadingDiff.value = true;
  diffError.value = null;
  try {
    const text = await api.gitFileDiff(projectRoot.value, path);
    diffCache.value.set(path, text);
    // Map não é reativo em profundidade sozinho pra `computed` — força um novo Map.
    diffCache.value = new Map(diffCache.value);
  } catch (e) {
    diffError.value = String(e);
  } finally {
    loadingDiff.value = false;
  }
}

const selectedDiffLines = computed(() => {
  if (!selectedPath.value) return [];
  const text = diffCache.value.get(selectedPath.value);
  return text ? classifyDiffLines(text) : [];
});

watch(
  () => props.visible,
  (v) => {
    if (v && !selectedPath.value && changes.value.length > 0) {
      selectFile(changes.value[0].path);
    }
  },
);

function fileName(path: string): string {
  return path.split(/[/\\]/).filter(Boolean).pop() ?? path;
}

async function onRootPick(path: string) {
  await sessionStore.updateProjectRoot(path);
}
</script>

<template>
  <Dialog
    :visible="visible"
    @update:visible="(v) => emit('update:visible', v)"
    :header="$t('repoDiffViewer.title')"
    modal
    maximizable
    :style="{ width: '880px' }"
  >
    <div class="rdv-root-row">
      <FolderPicker :current="projectRoot" :allow-browse="false" @pick="onRootPick" />
    </div>

    <p v-if="statusError" class="hint rdv-empty">{{ statusError }}</p>
    <div v-else-if="loadingStatus" class="hint rdv-empty">{{ $t("repoDiffViewer.loading") }}</div>
    <div v-else-if="changes.length === 0" class="hint rdv-empty">{{ $t("repoDiffViewer.noChanges") }}</div>
    <div v-else class="rdv-body">
      <div class="rdv-file-list">
        <button
          v-for="change in changes"
          :key="change.path"
          class="rdv-file-item"
          :class="{ active: change.path === selectedPath }"
          @click="selectFile(change.path)"
        >
          <span class="rdv-status-badge" :class="`status-${change.status}`">{{ STATUS_LABELS[change.status] }}</span>
          <span class="rdv-file-name" :title="change.path">{{ fileName(change.path) }}</span>
          <span v-if="isPending(change.path)" class="rdv-pending-badge">{{ $t("repoDiffViewer.pending") }}</span>
          <span class="rdv-file-stats">
            <span v-if="change.additions" class="stat-add">+{{ change.additions }}</span>
            <span v-if="change.deletions" class="stat-del">-{{ change.deletions }}</span>
          </span>
        </button>
      </div>
      <div class="rdv-diff-pane">
        <template v-if="selectedPath">
          <p class="rdv-diff-path">{{ selectedPath }}</p>
          <p v-if="loadingDiff" class="hint">{{ $t("repoDiffViewer.loading") }}</p>
          <p v-else-if="diffError" class="hint">{{ diffError }}</p>
          <div v-else class="diff-box">
            <div
              v-for="(line, idx) in selectedDiffLines"
              :key="idx"
              class="diff-line"
              :class="`diff-${line.kind}`"
            >{{ line.text || " " }}</div>
          </div>
        </template>
        <p v-else class="hint">{{ $t("repoDiffViewer.selectFile") }}</p>
      </div>
    </div>
  </Dialog>
</template>

<style scoped>
.rdv-root-row {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-bottom: 10px;
}

.rdv-empty {
  padding: 20px 4px;
}

.rdv-body {
  display: flex;
  gap: 12px;
  height: 480px;
}

.rdv-file-list {
  width: 220px;
  flex-shrink: 0;
  overflow-y: auto;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 4px;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.rdv-file-item {
  display: flex;
  align-items: center;
  gap: 6px;
  border: none;
  background: transparent;
  border-radius: 6px;
  padding: 6px 8px;
  cursor: pointer;
  text-align: left;
  font-size: 12px;
}

.rdv-file-item:hover {
  background: #f4f4f5;
}

.rdv-file-item.active {
  background: #e4e4e7;
}

.rdv-status-badge {
  flex-shrink: 0;
  width: 16px;
  height: 16px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 4px;
  font-size: 10px;
  font-weight: 700;
  font-family: var(--cerne-mono);
  color: #ffffff;
  background: #71717a;
}

.rdv-status-badge.status-modified {
  background: #d97706;
}

.rdv-status-badge.status-added,
.rdv-status-badge.status-untracked {
  background: #16a34a;
}

.rdv-status-badge.status-deleted {
  background: #dc2626;
}

.rdv-status-badge.status-renamed {
  background: #6366f1;
}

.rdv-file-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--cerne-mono);
  color: #18181b;
}

.rdv-pending-badge {
  flex-shrink: 0;
  font-size: 9px;
  font-weight: 700;
  color: #b45309;
  background: #fef3c7;
  border-radius: 4px;
  padding: 1px 4px;
}

.rdv-file-stats {
  flex-shrink: 0;
  display: flex;
  gap: 4px;
  font-size: 10px;
  font-weight: 600;
  font-family: var(--cerne-mono);
}

.stat-add {
  color: #16a34a;
}

.stat-del {
  color: #dc2626;
}

.rdv-diff-pane {
  flex: 1;
  overflow-y: auto;
  min-width: 0;
}

.rdv-diff-path {
  margin: 0 0 8px;
  font-size: 12px;
  font-weight: 600;
  font-family: var(--cerne-mono);
  color: #3f3f46;
  overflow-wrap: break-word;
}

.diff-box {
  background: #1e1e1e;
  border-radius: 8px;
  padding: 8px 10px;
  overflow-x: auto;
  font-size: 12px;
  font-family: var(--cerne-mono);
}

.diff-line {
  white-space: pre;
  color: #d4d4d8;
}

.diff-add {
  background: rgba(34, 197, 94, 0.15);
  color: #86efac;
}

.diff-del {
  background: rgba(239, 68, 68, 0.15);
  color: #fca5a5;
}

.diff-hunk {
  color: #60a5fa;
}

.diff-header {
  color: #9d9d9d;
}

.hint {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
}
</style>
