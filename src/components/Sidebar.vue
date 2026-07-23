<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { useSessionStore } from "../stores/session";

const props = defineProps<{ view: "chat" | "settings" }>();
const emit = defineEmits<{
  "update:view": [value: "chat" | "settings"];
  "new-session": [];
  "open-help": [];
}>();

const sessionStore = useSessionStore();
const search = ref("");
const collapsed = ref(false);
const editingId = ref<string | null>(null);
const editingTitle = ref("");
const renameInputRef = ref<HTMLInputElement | null>(null);

const filtered = computed(() =>
  sessionStore.sessions.filter((s) => s.title.toLowerCase().includes(search.value.toLowerCase())),
);

function select(id: string) {
  emit("update:view", "chat");
  sessionStore.selectSession(id);
}

async function startRename(id: string, currentTitle: string) {
  editingId.value = id;
  editingTitle.value = currentTitle;
  await nextTick();
  renameInputRef.value?.focus();
  renameInputRef.value?.select();
}

async function confirmRename() {
  if (!editingId.value) return;
  await sessionStore.updateTitle(editingId.value, editingTitle.value);
  editingId.value = null;
}

function cancelRename() {
  editingId.value = null;
}

async function remove(id: string) {
  await sessionStore.deleteSession(id);
}
</script>

<template>
  <aside class="sidebar" :class="{ collapsed }">
    <div class="top-row">
      <button class="icon-btn" @click="collapsed = !collapsed" v-tooltip.right="'Recolher'">
        <span class="msi">dock_to_right</span>
      </button>
      <span v-if="!collapsed" class="brand">Cerne Code</span>
    </div>

    <button class="new-session" @click="emit('new-session')">
      <span class="msi">add</span>
      <span v-if="!collapsed">Nova sessão</span>
    </button>

    <template v-if="!collapsed">
      <div class="search-box">
        <span class="msi">search</span>
        <input v-model="search" placeholder="Buscar sessões" />
      </div>

      <div class="section-label">Recentes</div>
      <div class="session-list">
        <div
          v-for="s in filtered"
          :key="s.id"
          class="session-item"
          :class="{ active: s.id === sessionStore.currentId && props.view === 'chat' }"
          @click="editingId !== s.id && select(s.id)"
        >
          <span
            class="msi session-icon"
            :class="{ 'code-icon': s.project_root }"
            v-tooltip.right="s.project_root ? `Código — ${s.project_root}` : 'Chat'"
          >{{ s.project_root ? "terminal" : "chat_bubble" }}</span>
          <input
            v-if="editingId === s.id"
            ref="renameInputRef"
            v-model="editingTitle"
            class="session-rename-input"
            @click.stop
            @keydown.enter="confirmRename"
            @keydown.escape="cancelRename"
            @blur="confirmRename"
          />
          <span v-else class="session-title">{{ s.title }}</span>
          <div v-if="editingId === s.id" class="session-actions editing">
            <button class="session-action-btn" v-tooltip.top="'Salvar'" @mousedown.prevent="confirmRename">
              <span class="msi">save</span>
            </button>
          </div>
          <div v-else class="session-actions">
            <button class="session-action-btn" v-tooltip.top="'Renomear'" @click.stop="startRename(s.id, s.title)">
              <span class="msi">edit</span>
            </button>
            <button class="session-action-btn" v-tooltip.top="'Excluir'" @click.stop="remove(s.id)">
              <span class="msi">delete</span>
            </button>
          </div>
        </div>
        <p v-if="filtered.length === 0" class="empty">Nenhuma sessão ainda.</p>
      </div>
    </template>

    <div class="bottom-row">
      <button class="icon-btn" @click="emit('open-help')" v-tooltip.right="'Ajuda'">
        <span class="msi">help</span>
        <span v-if="!collapsed">Ajuda</span>
      </button>
      <button
        class="icon-btn"
        :class="{ active: props.view === 'settings' }"
        @click="emit('update:view', 'settings')"
        v-tooltip.right="'Configurações'"
      >
        <span class="msi">settings</span>
        <span v-if="!collapsed">Configurações</span>
      </button>
    </div>
  </aside>
</template>

<style scoped>
.sidebar {
  width: var(--cerne-sidebar-width);
  border-right: var(--cerne-border);
  display: flex;
  flex-direction: column;
  padding: 10px 8px;
  gap: 4px;
  flex-shrink: 0;
  transition: width 0.15s ease;
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

.bottom-row {
  border-top: var(--cerne-border);
  padding-top: 6px;
}
</style>
