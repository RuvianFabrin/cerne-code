<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import { useSessionStore } from "../stores/session";
import SidebarSessionItem from "./SidebarSessionItem.vue";
import SidebarSessionTree from "./SidebarSessionTree.vue";
import type { Session } from "../api";

const emit = defineEmits<{
  "new-session": [folderId?: string | null];
  "open-help": [];
  "open-about": [];
  "open-settings": [];
  "open-agents-skills": [];
}>();

const { t } = useI18n();
const sessionStore = useSessionStore();
const search = ref("");
const collapsed = ref(false);

// Largura da sidebar é arrastável (handle na borda direita) e persiste entre
// reinícios — sem isso, todo mundo fica preso na largura fixa original,
// ruim pra quem tem título de sessão longo ou tela pequena.
const MIN_SIDEBAR_WIDTH = 200;
const MAX_SIDEBAR_WIDTH = 480;
const DEFAULT_SIDEBAR_WIDTH = 272;
const SIDEBAR_WIDTH_KEY = "cerne-sidebar-width";

function loadStoredWidth(): number {
  const stored = Number(localStorage.getItem(SIDEBAR_WIDTH_KEY));
  if (Number.isFinite(stored) && stored >= MIN_SIDEBAR_WIDTH && stored <= MAX_SIDEBAR_WIDTH) return stored;
  return DEFAULT_SIDEBAR_WIDTH;
}

const sidebarWidth = ref(loadStoredWidth());
const resizing = ref(false);

function startResize(e: MouseEvent) {
  e.preventDefault();
  resizing.value = true;
  const startX = e.clientX;
  const startWidth = sidebarWidth.value;

  function onMove(moveEvent: MouseEvent) {
    const next = startWidth + (moveEvent.clientX - startX);
    sidebarWidth.value = Math.min(MAX_SIDEBAR_WIDTH, Math.max(MIN_SIDEBAR_WIDTH, next));
  }
  function onUp() {
    resizing.value = false;
    localStorage.setItem(SIDEBAR_WIDTH_KEY, String(sidebarWidth.value));
    window.removeEventListener("mousemove", onMove);
    window.removeEventListener("mouseup", onUp);
  }
  window.addEventListener("mousemove", onMove);
  window.addEventListener("mouseup", onUp);
}

// Excluir sessão é destrutivo e irreversível (apaga o histórico do chat
// inteiro) — confirma antes, em vez de excluir no primeiro clique.
const deleteTarget = ref<{ id: string; title: string } | null>(null);

function requestDelete(id: string, title: string) {
  deleteTarget.value = { id, title };
}

async function confirmDelete() {
  if (!deleteTarget.value) return;
  await sessionStore.deleteSession(deleteTarget.value.id);
  deleteTarget.value = null;
}

// Pasta que o usuário pediu pra excluir — sessões/subpastas dela sobem pra
// raiz automaticamente (reversível arrastando de volta), sem exigir uma
// segunda confirmação além do próprio clique no ícone de excluir.
const deleteFolderTarget = ref<{ id: string; name: string } | null>(null);

function requestDeleteFolder(id: string, name: string) {
  deleteFolderTarget.value = { id, name };
}

async function confirmDeleteFolder() {
  if (!deleteFolderTarget.value) return;
  await sessionStore.deleteFolder(deleteFolderTarget.value.id);
  deleteFolderTarget.value = null;
}

const filtered = computed(() =>
  sessionStore.sessions.filter((s) => s.title.toLowerCase().includes(search.value.toLowerCase())),
);

// Buscando: achata a árvore (ignora pastas, mostra resultados soltos) — faz
// mais sentido que filtrar mantendo a hierarquia intacta.
const isSearching = computed(() => search.value.trim().length > 0);

const rootFolders = computed(() =>
  sessionStore.folders.filter((f) => !f.parent_id).sort((a, b) => a.name.localeCompare(b.name)),
);

function subfoldersOf(rootId: string) {
  return sessionStore.folders
    .filter((f) => f.parent_id === rootId)
    .sort((a, b) => a.name.localeCompare(b.name));
}

const knownFolderIds = computed(() => new Set(sessionStore.folders.map((f) => f.id)));

// Lista achatada em ordem de árvore (pai sempre antes dos próprios filhos,
// recursivo) — pro dropdown "mover sessão" de SidebarSessionItem, que só
// recebe um array flat e não pode construir a árvore sozinho. Sem isso o
// dropdown usava `sessionStore.folders` cru (ordem de chegada da API), então
// uma subpasta podia aparecer longe da pasta-mãe dela na lista.
const sortedFolders = computed(() => {
  const result: typeof sessionStore.folders = [];
  function addChildren(parentId: string | null) {
    const children = sessionStore.folders
      .filter((f) => f.parent_id === parentId)
      .sort((a, b) => a.name.localeCompare(b.name));
    for (const f of children) {
      result.push(f);
      addChildren(f.id);
    }
  }
  addChildren(null);
  return result;
});

// Sessão orquestrada (com parent_session_id) nunca aparece como entrada solta
// na lista/pasta dela mesma — só nasce visível quando a sessão-mãe é
// expandida (G4.1), não importa em qual pasta ela caiu. Exceção: se a
// sessão-mãe não existe mais (foi excluída), a filha órfã volta a aparecer
// normal, senão ela some da sidebar pra sempre sem jeito de abrir.
const knownSessionIds = computed(() => new Set(sessionStore.sessions.map((s) => s.id)));
function isNestedChild(s: Session): boolean {
  return !!s.parent_session_id && knownSessionIds.value.has(s.parent_session_id);
}

function sessionsIn(folderId: string): Session[] {
  return filtered.value.filter((s) => s.folder_id === folderId && !isNestedChild(s));
}

const looseSessions = computed(() =>
  filtered.value.filter(
    (s) => (!s.folder_id || !knownFolderIds.value.has(s.folder_id)) && !isNestedChild(s),
  ),
);

// G4.1: árvore expansível pra sessões orquestradas — mesmo padrão de
// localStorage já usado pras pastas (expandedFolders acima), estado de UI
// puro, não vale a pena mandar pro backend.
const EXPANDED_SESSIONS_KEY = "cerne-sidebar-expanded-sessions";

function loadExpandedSessions(): Set<string> {
  try {
    const raw = localStorage.getItem(EXPANDED_SESSIONS_KEY);
    return raw ? new Set(JSON.parse(raw)) : new Set();
  } catch {
    return new Set();
  }
}

const expandedSessions = ref<Set<string>>(loadExpandedSessions());

function isSessionExpanded(id: string) {
  return expandedSessions.value.has(id);
}

function toggleSessionExpanded(id: string) {
  const next = new Set(expandedSessions.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  expandedSessions.value = next;
  localStorage.setItem(EXPANDED_SESSIONS_KEY, JSON.stringify([...next]));
}

// Estado de expandido/recolhido é preferência de UI local — não vale a pena
// ir pro backend, só persiste em localStorage entre reinícios.
const EXPANDED_FOLDERS_KEY = "cerne-sidebar-expanded-folders";

function loadExpandedFolders(): Set<string> {
  try {
    const raw = localStorage.getItem(EXPANDED_FOLDERS_KEY);
    return raw ? new Set(JSON.parse(raw)) : new Set();
  } catch {
    return new Set();
  }
}

const expandedFolders = ref<Set<string>>(loadExpandedFolders());

function isExpanded(id: string) {
  return expandedFolders.value.has(id);
}

function toggleFolder(id: string) {
  const next = new Set(expandedFolders.value);
  if (next.has(id)) next.delete(id);
  else next.add(id);
  expandedFolders.value = next;
  localStorage.setItem(EXPANDED_FOLDERS_KEY, JSON.stringify([...next]));
}

function expandFolder(id: string) {
  if (expandedFolders.value.has(id)) return;
  toggleFolder(id);
}

// Criar pasta (raiz ou subpasta): mesmo padrão de edição inline já usado pra
// renomear sessão — abre um input no lugar, confirma no Enter/blur.
const creatingFolderParentId = ref<string | null | undefined>(undefined);
const newFolderName = ref("");
const newFolderInputRef = ref<HTMLInputElement | null>(null);

async function startCreateFolder(parentId: string | null) {
  creatingFolderParentId.value = parentId;
  newFolderName.value = "";
  if (parentId) expandFolder(parentId);
  await nextTick();
  newFolderInputRef.value?.focus();
}

async function confirmCreateFolder() {
  if (creatingFolderParentId.value === undefined) return;
  const name = newFolderName.value.trim();
  const parentId = creatingFolderParentId.value;
  creatingFolderParentId.value = undefined;
  if (!name) return;
  const folder = await sessionStore.createFolder(name, parentId);
  expandFolder(folder.id);
  if (parentId) expandFolder(parentId);
}

function cancelCreateFolder() {
  creatingFolderParentId.value = undefined;
}

// Renomear pasta: mesmo padrão inline.
const editingFolderId = ref<string | null>(null);
const editingFolderName = ref("");
const editFolderInputRef = ref<HTMLInputElement | null>(null);

async function startRenameFolder(id: string, currentName: string) {
  editingFolderId.value = id;
  editingFolderName.value = currentName;
  await nextTick();
  editFolderInputRef.value?.focus();
  editFolderInputRef.value?.select();
}

async function confirmRenameFolder() {
  if (!editingFolderId.value) return;
  const id = editingFolderId.value;
  editingFolderId.value = null;
  await sessionStore.renameFolder(id, editingFolderName.value);
}

function cancelRenameFolder() {
  editingFolderId.value = null;
}
</script>

<template>
  <aside
    class="sidebar"
    :class="{ collapsed, resizing }"
    :style="collapsed ? {} : { width: sidebarWidth + 'px' }"
  >
    <div class="top-row">
      <button class="icon-btn" @click="collapsed = !collapsed" v-tooltip.right="$t('sidebar.collapse')">
        <span class="msi">dock_to_right</span>
      </button>
      <span v-if="!collapsed" class="brand">Cerne Code</span>
    </div>

    <button class="new-session" @click="emit('new-session')">
      <span class="msi">add</span>
      <span v-if="!collapsed">{{ $t("sidebar.newSession") }}</span>
    </button>

    <template v-if="!collapsed">
      <div class="search-box">
        <span class="msi">search</span>
        <input v-model="search" :placeholder="$t('sidebar.searchPlaceholder')" />
      </div>

      <div class="section-label-row">
        <span class="section-label">{{ $t("sidebar.recent") }}</span>
        <button
          v-if="!isSearching"
          class="folder-add-btn"
          v-tooltip.top="$t('sidebar.newFolder')"
          @click="startCreateFolder(null)"
        >
          <span class="msi">create_new_folder</span>
        </button>
      </div>

      <div class="session-list">
        <!-- Buscando: lista achatada, sem árvore de pastas. -->
        <template v-if="isSearching">
          <SidebarSessionItem
            v-for="s in filtered"
            :key="s.id"
            :session="s"
            :active="s.id === sessionStore.currentId"
            :folders="sortedFolders"
            @delete="requestDelete"
          />
          <p v-if="filtered.length === 0" class="empty">{{ $t("sidebar.empty") }}</p>
        </template>

        <template v-else>
          <div v-if="creatingFolderParentId === null" class="folder-create-row">
            <span class="msi">folder</span>
            <input
              ref="newFolderInputRef"
              v-model="newFolderName"
              class="session-rename-input"
              :placeholder="$t('sidebar.folderNamePlaceholder')"
              @keydown.enter="confirmCreateFolder"
              @keydown.escape="cancelCreateFolder"
              @blur="confirmCreateFolder"
            />
          </div>

          <div v-for="folder in rootFolders" :key="folder.id" class="folder-group">
            <div class="folder-row" @click="toggleFolder(folder.id)">
              <span class="msi folder-caret">{{ isExpanded(folder.id) ? "expand_more" : "chevron_right" }}</span>
              <span class="msi folder-icon">folder</span>
              <input
                v-if="editingFolderId === folder.id"
                ref="editFolderInputRef"
                v-model="editingFolderName"
                class="session-rename-input"
                @click.stop
                @keydown.enter="confirmRenameFolder"
                @keydown.escape="cancelRenameFolder"
                @blur="confirmRenameFolder"
              />
              <span v-else class="folder-title">{{ folder.name }}</span>
              <div class="session-actions" @click.stop>
                <button class="session-action-btn" v-tooltip.top="$t('sidebar.newSession')" @click="emit('new-session', folder.id); expandFolder(folder.id)">
                  <span class="msi">add</span>
                </button>
                <button class="session-action-btn" v-tooltip.top="$t('sidebar.newSubfolder')" @click="startCreateFolder(folder.id)">
                  <span class="msi">create_new_folder</span>
                </button>
                <button class="session-action-btn" v-tooltip.top="$t('sidebar.rename')" @click="startRenameFolder(folder.id, folder.name)">
                  <span class="msi">edit</span>
                </button>
                <button class="session-action-btn" v-tooltip.top="$t('sidebar.delete')" @click="requestDeleteFolder(folder.id, folder.name)">
                  <span class="msi">delete</span>
                </button>
              </div>
            </div>

            <div v-if="isExpanded(folder.id)" class="folder-children">
              <div v-if="creatingFolderParentId === folder.id" class="folder-create-row nested">
                <span class="msi">folder</span>
                <input
                  ref="newFolderInputRef"
                  v-model="newFolderName"
                  class="session-rename-input"
                  :placeholder="$t('sidebar.folderNamePlaceholder')"
                  @keydown.enter="confirmCreateFolder"
                  @keydown.escape="cancelCreateFolder"
                  @blur="confirmCreateFolder"
                />
              </div>

              <SidebarSessionTree
                v-for="s in sessionsIn(folder.id)"
                :key="s.id"
                :session="s"
                :depth="0"
                :folders="sortedFolders"
                :is-expanded="isSessionExpanded"
                @delete="requestDelete"
                @toggle="toggleSessionExpanded"
              />

              <div v-for="sub in subfoldersOf(folder.id)" :key="sub.id" class="folder-group nested">
                <div class="folder-row" @click="toggleFolder(sub.id)">
                  <span class="msi folder-caret">{{ isExpanded(sub.id) ? "expand_more" : "chevron_right" }}</span>
                  <span class="msi folder-icon">folder</span>
                  <input
                    v-if="editingFolderId === sub.id"
                    ref="editFolderInputRef"
                    v-model="editingFolderName"
                    class="session-rename-input"
                    @click.stop
                    @keydown.enter="confirmRenameFolder"
                    @keydown.escape="cancelRenameFolder"
                    @blur="confirmRenameFolder"
                  />
                  <span v-else class="folder-title">{{ sub.name }}</span>
                  <div class="session-actions" @click.stop>
                    <button class="session-action-btn" v-tooltip.top="$t('sidebar.newSession')" @click="emit('new-session', sub.id); expandFolder(sub.id)">
                      <span class="msi">add</span>
                    </button>
                    <button class="session-action-btn" v-tooltip.top="$t('sidebar.rename')" @click="startRenameFolder(sub.id, sub.name)">
                      <span class="msi">edit</span>
                    </button>
                    <button class="session-action-btn" v-tooltip.top="$t('sidebar.delete')" @click="requestDeleteFolder(sub.id, sub.name)">
                      <span class="msi">delete</span>
                    </button>
                  </div>
                </div>
                <div v-if="isExpanded(sub.id)" class="folder-children">
                  <SidebarSessionTree
                    v-for="s in sessionsIn(sub.id)"
                    :key="s.id"
                    :session="s"
                    :depth="0"
                    :folders="sortedFolders"
                    :is-expanded="isSessionExpanded"
                    @delete="requestDelete"
                    @toggle="toggleSessionExpanded"
                  />
                  <p v-if="sessionsIn(sub.id).length === 0" class="empty nested">{{ $t("sidebar.emptyFolder") }}</p>
                </div>
              </div>
            </div>
          </div>

          <SidebarSessionTree
            v-for="s in looseSessions"
            :key="s.id"
            :session="s"
            :depth="0"
            :folders="sortedFolders"
            :is-expanded="isSessionExpanded"
            @delete="requestDelete"
            @toggle="toggleSessionExpanded"
          />
          <p v-if="rootFolders.length === 0 && looseSessions.length === 0 && creatingFolderParentId === undefined" class="empty">{{ $t("sidebar.empty") }}</p>
        </template>
      </div>
    </template>

    <div class="bottom-row">
      <button class="icon-btn" @click="emit('open-agents-skills')" v-tooltip.right="$t('sidebar.agentsSkills')">
        <span class="msi">smart_toy</span>
        <span v-if="!collapsed">{{ $t("sidebar.agentsSkills") }}</span>
      </button>
      <button class="icon-btn" @click="emit('open-help')" v-tooltip.right="$t('sidebar.help')">
        <span class="msi">help</span>
        <span v-if="!collapsed">{{ $t("sidebar.help") }}</span>
      </button>
      <button
        class="icon-btn"
        @click="emit('open-settings')"
        v-tooltip.right="$t('sidebar.settings')"
      >
        <span class="msi">settings</span>
        <span v-if="!collapsed">{{ $t("sidebar.settings") }}</span>
      </button>
      <button class="icon-btn" @click="emit('open-about')" v-tooltip.right="$t('sidebar.about')">
        <span class="msi">info</span>
        <span v-if="!collapsed">{{ $t("sidebar.about") }}</span>
      </button>
    </div>

    <div
      v-if="!collapsed"
      class="resize-handle"
      v-tooltip.right="$t('sidebar.resizeTooltip')"
      @mousedown="startResize"
    ></div>
  </aside>

  <Dialog
    :visible="!!deleteTarget"
    @update:visible="(v) => { if (!v) deleteTarget = null; }"
    modal
    :header="t('sidebar.deleteConfirmTitle')"
    :style="{ width: 'min(420px, 92vw)' }"
  >
    <p class="delete-confirm-text">{{ t("sidebar.deleteConfirmBody", { title: deleteTarget?.title ?? "" }) }}</p>
    <template #footer>
      <button class="btn-secondary" @click="deleteTarget = null">{{ t("newSession.cancel") }}</button>
      <button class="btn-danger" @click="confirmDelete">{{ t("sidebar.delete") }}</button>
    </template>
  </Dialog>

  <Dialog
    :visible="!!deleteFolderTarget"
    @update:visible="(v) => { if (!v) deleteFolderTarget = null; }"
    modal
    :header="t('sidebar.deleteFolderConfirmTitle')"
    :style="{ width: 'min(420px, 92vw)' }"
  >
    <p class="delete-confirm-text">{{ t("sidebar.deleteFolderConfirmBody", { name: deleteFolderTarget?.name ?? "" }) }}</p>
    <template #footer>
      <button class="btn-secondary" @click="deleteFolderTarget = null">{{ t("newSession.cancel") }}</button>
      <button class="btn-danger" @click="confirmDeleteFolder">{{ t("sidebar.delete") }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.sidebar {
  position: relative;
  width: var(--cerne-sidebar-width);
  border-right: var(--cerne-border);
  display: flex;
  flex-direction: column;
  padding: 10px 8px;
  gap: 4px;
  flex-shrink: 0;
  transition: width 0.15s ease;
}

.sidebar.resizing {
  transition: none;
  user-select: none;
}

.sidebar.collapsed {
  width: 56px;
  align-items: center;
}

.top-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 6px 10px;
}

.brand {
  font-weight: 600;
  font-size: 14px;
  color: #18181b;
  white-space: nowrap;
}

.icon-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  border: none;
  background: transparent;
  color: #52525b;
  padding: 7px 8px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  width: 100%;
  text-align: left;
}

.icon-btn:hover,
.icon-btn.active {
  background: #f4f4f5;
  color: #18181b;
}

.new-session {
  display: flex;
  align-items: center;
  gap: 8px;
  border: var(--cerne-border);
  background: #ffffff;
  color: #18181b;
  padding: 8px 10px;
  border-radius: 8px;
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  margin-bottom: 6px;
}

.new-session:hover {
  background: #fafafa;
}

.search-box {
  display: flex;
  align-items: center;
  gap: 6px;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 6px 8px;
  margin-bottom: 4px;
  color: #71717a;
}

.search-box input {
  border: none;
  outline: none;
  font-size: 13px;
  font-weight: 500;
  flex: 1;
  color: #18181b;
  background: transparent;
}

.search-box .msi {
  font-size: 16px;
}

.section-label {
  font-size: 11px;
  font-weight: 600;
  color: #a1a1aa;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  padding: 8px 8px 4px;
}

.session-list {
  flex: 1;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.session-item {
  display: flex;
  align-items: center;
  gap: 8px;
  border: none;
  background: transparent;
  padding: 7px 8px;
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

.empty {
  font-size: 12px;
  color: #a1a1aa;
  padding: 8px;
}

.empty.nested {
  padding: 4px 8px 4px 30px;
}

.section-label-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-right: 4px;
}

.section-label-row .section-label {
  padding-right: 0;
}

.folder-add-btn {
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

.folder-add-btn:hover {
  background: #e4e4e7;
  color: #18181b;
}

.folder-add-btn .msi {
  font-size: 15px;
}

.folder-group {
  display: flex;
  flex-direction: column;
}

.folder-group.nested {
  margin-left: 18px;
}

.folder-row {
  display: flex;
  align-items: center;
  gap: 4px;
  border: none;
  background: transparent;
  padding: 3px 8px;
  border-radius: 6px;
  cursor: pointer;
  text-align: left;
  color: #3f3f46;
  font-size: 13px;
  font-weight: 500;
}

.folder-row:hover {
  background: #f4f4f5;
}

.folder-row .msi {
  font-size: 16px;
  color: #a1a1aa;
  flex-shrink: 0;
}

.folder-caret {
  font-size: 15px !important;
}

.folder-title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.folder-row .session-actions {
  margin-left: auto;
}

.folder-row:hover .session-actions {
  opacity: 1;
}

.folder-children {
  display: flex;
  flex-direction: column;
  gap: 1px;
  margin-left: 18px;
}

.folder-create-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 8px;
}

.folder-create-row.nested {
  margin-left: 18px;
}

.folder-create-row .msi {
  font-size: 16px;
  color: #a1a1aa;
}

.bottom-row {
  border-top: var(--cerne-border);
  padding-top: 6px;
}

.resize-handle {
  position: absolute;
  top: 0;
  right: -3px;
  width: 6px;
  height: 100%;
  cursor: col-resize;
  z-index: 5;
}

.resize-handle:hover,
.sidebar.resizing .resize-handle {
  background: var(--cerne-accent, #6366f1);
  opacity: 0.4;
}

.delete-confirm-text {
  font-size: 13px;
  color: #3f3f46;
  line-height: 1.5;
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
}

.btn-danger {
  border: none;
  background: #dc2626;
  color: #ffffff;
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.btn-danger:hover {
  background: #b91c1c;
}
</style>
