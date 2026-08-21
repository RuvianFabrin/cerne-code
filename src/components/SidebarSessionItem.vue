<script setup lang="ts">
// Linha de sessão reaproveitada em 3 lugares na árvore da Sidebar (T29):
// soltas na raiz, dentro de pasta de raiz, dentro de subpasta. Extraída pra
// não triplicar a lógica de renomear/excluir/mover a cada nível.
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import type { Folder, Session } from "../api";
import { useSessionStore } from "../stores/session";

const props = defineProps<{
  session: Session;
  active: boolean;
  folders: Folder[];
}>();

// Fase G: sessão criada por `start_agent_session` (sessão paralela
// orquestrada) — badge só pra deixar claro de onde ela veio, sem UI
// especial nova (ela já aparece na lista normal, só clicar como qualquer
// outra sessão).
const parentSessionTitle = computed(() => {
  if (!props.session.parent_session_id) return null;
  return sessionStore.sessions.find((s) => s.id === props.session.parent_session_id)?.title ?? null;
});

// Pedido do usuário (2026-08-18): bolinha piscando indicando que ESSA
// sessão tem um turno rodando agora, mesmo se não for a sessão aberta no
// momento — dá pra ter várias rodando ao mesmo tempo (API roda em
// paralelo de verdade; local serializa, mas ainda mostra "processando" até
// a fila resolver).
const isProcessing = computed(() => sessionStore.processingSessionIds.has(props.session.id));

const emit = defineEmits<{
  delete: [id: string, title: string];
}>();

const { t } = useI18n();
const sessionStore = useSessionStore();
const editing = ref(false);
const editingTitle = ref("");
const renameInputRef = ref<HTMLInputElement | null>(null);
const movingOpen = ref(false);

function select() {
  if (!editing.value) sessionStore.selectSession(props.session.id);
}

// `props.folders` já vem em ordem de árvore (pai antes dos filhos), mas a
// indentação do dropdown precisa da profundidade real (não só "tem pai ou
// não") pra ficar certa em qualquer nível — sobe a cadeia de parent_id
// dentro da própria lista recebida.
function folderDepth(f: Folder): number {
  let depth = 0;
  let current = f;
  while (current.parent_id) {
    const parent = props.folders.find((p) => p.id === current.parent_id);
    if (!parent) break;
    depth += 1;
    current = parent;
  }
  return depth;
}

async function startRename() {
  editing.value = true;
  editingTitle.value = props.session.title;
  await nextTick();
  renameInputRef.value?.focus();
  renameInputRef.value?.select();
}

async function confirmRename() {
  if (!editing.value) return;
  editing.value = false;
  await sessionStore.updateTitle(props.session.id, editingTitle.value);
}

function cancelRename() {
  editing.value = false;
}

async function moveTo(folderId: string) {
  movingOpen.value = false;
  const target = folderId === "__root__" ? null : folderId;
  if (target === props.session.folder_id) return;
  await sessionStore.moveSessionToFolder(props.session.id, target);
}
</script>

<template>
  <div
    class="session-item"
    :class="{ active }"
    @click="select"
  >
    <span
      class="msi session-icon"
      :class="{ 'code-icon': session.project_root || session.extra_read_paths?.length }"
      v-tooltip.right="(session.project_root || session.extra_read_paths?.length) ? $t('sidebar.codeTooltip', { path: session.project_root || session.extra_read_paths?.[0]?.path || '' }) : $t('sidebar.chatTooltip')"
    >{{ (session.project_root || session.extra_read_paths?.length) ? "terminal" : "chat_bubble" }}</span>
    <input
      v-if="editing"
      ref="renameInputRef"
      v-model="editingTitle"
      class="session-rename-input"
      @click.stop
      @keydown.enter="confirmRename"
      @keydown.escape="cancelRename"
      @blur="confirmRename"
    />
    <span v-else class="session-title">{{ session.title }}</span>
    <span
      v-if="isProcessing"
      class="processing-dot"
      v-tooltip.right="$t('sidebar.processingTooltip')"
    />
    <span
      v-if="parentSessionTitle"
      class="msi orchestrated-badge"
      v-tooltip.right="$t('sidebar.orchestratedTooltip', { title: parentSessionTitle })"
    >account_tree</span>

    <div v-if="editing" class="session-actions editing">
      <button class="session-action-btn" v-tooltip.top="$t('sidebar.save')" @mousedown.prevent="confirmRename">
        <span class="msi">save</span>
      </button>
    </div>
    <div v-else class="session-actions" @click.stop>
      <div class="move-wrap">
        <button
          class="session-action-btn"
          v-tooltip.top="$t('sidebar.moveTo')"
          @click="movingOpen = !movingOpen"
        >
          <span class="msi">drive_file_move</span>
        </button>
        <div v-if="movingOpen" class="move-menu" @click.stop>
          <button
            class="move-menu-item"
            :class="{ current: !session.folder_id }"
            @click="moveTo('__root__')"
          >{{ t("sidebar.rootFolder") }}</button>
          <template v-for="f in folders" :key="f.id">
            <button
              class="move-menu-item"
              :class="{ current: session.folder_id === f.id }"
              @click="moveTo(f.id)"
            >{{ folderDepth(f) > 0 ? `${"— ".repeat(folderDepth(f))}${f.name}` : f.name }}</button>
          </template>
        </div>
      </div>
      <button class="session-action-btn" v-tooltip.top="$t('sidebar.rename')" @click="startRename">
        <span class="msi">edit</span>
      </button>
      <button class="session-action-btn" v-tooltip.top="$t('sidebar.delete')" @click="emit('delete', session.id, session.title)">
        <span class="msi">delete</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.session-item {
  display: flex;
  align-items: center;
  gap: 8px;
  border: none;
  background: transparent;
  padding: 4px 8px;
  border-radius: 6px;
  cursor: pointer;
  text-align: left;
  color: #3f3f46;
  font-size: 13px;
  font-weight: 500;
}

.session-item .msi {
  font-size: 16px;
  color: #a1a1aa;
  flex-shrink: 0;
}

.session-icon.code-icon {
  color: var(--cerne-accent, #6366f1);
}

.session-rename-input {
  flex: 1;
  min-width: 0;
  border: 1px solid #18181b;
  border-radius: 4px;
  padding: 1px 4px;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  color: #18181b;
  outline: none;
  background: #ffffff;
}

.session-actions {
  display: flex;
  gap: 2px;
  margin-left: auto;
  flex-shrink: 0;
  opacity: 0;
  position: relative;
}

.session-item:hover .session-actions {
  opacity: 1;
}

.session-actions.editing {
  opacity: 1;
}

.session-action-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: #a1a1aa;
  padding: 3px;
  border-radius: 4px;
  cursor: pointer;
}

.session-action-btn:hover {
  background: #e4e4e7;
  color: #18181b;
}

.session-action-btn .msi {
  font-size: 15px;
}

.session-item:hover {
  background: #f4f4f5;
}

.session-item.active {
  background: #f4f4f5;
  color: #18181b;
}

.session-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.orchestrated-badge {
  flex-shrink: 0;
  font-size: 14px !important;
  color: var(--cerne-accent, #6366f1) !important;
}

.processing-dot {
  flex-shrink: 0;
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: #16a34a;
  animation: processing-dot-pulse 1.2s ease-in-out infinite;
}

@keyframes processing-dot-pulse {
  0%, 100% {
    opacity: 1;
    transform: scale(1);
  }
  50% {
    opacity: 0.35;
    transform: scale(0.7);
  }
}

.move-wrap {
  position: relative;
}

.move-menu {
  position: absolute;
  top: 100%;
  right: 0;
  z-index: 20;
  background: #ffffff;
  border: var(--cerne-border);
  border-radius: 8px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  padding: 4px;
  min-width: 160px;
  max-width: 240px;
  max-height: 240px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.move-menu-item {
  border: none;
  background: transparent;
  text-align: left;
  padding: 6px 8px;
  border-radius: 5px;
  cursor: pointer;
  font-size: 12px;
  font-weight: 500;
  color: #3f3f46;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.move-menu-item:hover {
  background: #f4f4f5;
}

.move-menu-item.current {
  color: var(--cerne-accent, #6366f1);
  font-weight: 600;
}
</style>
