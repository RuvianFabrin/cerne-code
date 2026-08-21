<script setup lang="ts">
// G4.1 (Fase G, PLANOS/13_roteiro_agentes_skills_fases.md): árvore
// expansível pra sessões orquestradas (criadas por start_agent_session) —
// ícone na sessão-mãe expande mostrando as filhas indentadas abaixo, e se
// uma filha também tiver filhas, mostra uma debaixo da outra recursivamente.
// Clicar de novo no ícone fecha tudo. Componente próprio (em vez de mexer
// direto em SidebarSessionItem) porque ele PRECISA se re-renderizar
// recursivamente pra suportar netos/bisnetos de sessão orquestrada, e
// SidebarSessionItem é reaproveitado em lugares (busca) que não devem ter
// árvore nenhuma.
import { computed } from "vue";
import type { Folder, Session } from "../api";
import { useSessionStore } from "../stores/session";
import SidebarSessionItem from "./SidebarSessionItem.vue";

const props = defineProps<{
  session: Session;
  folders: Folder[];
  depth: number;
  isExpanded: (id: string) => boolean;
}>();

const emit = defineEmits<{
  delete: [id: string, title: string];
  toggle: [id: string];
}>();

const sessionStore = useSessionStore();

const children = computed(() =>
  sessionStore.sessions.filter((s) => s.parent_session_id === props.session.id),
);
const expanded = computed(() => props.isExpanded(props.session.id));
</script>

<template>
  <div class="tree-node">
    <div class="tree-row" :style="{ paddingLeft: `${depth * 18}px` }">
      <button
        v-if="children.length > 0"
        class="tree-toggle"
        v-tooltip.top="$t(expanded ? 'sidebar.collapseChildren' : 'sidebar.expandChildren')"
        @click.stop="emit('toggle', session.id)"
      >
        <span class="msi">{{ expanded ? "expand_more" : "chevron_right" }}</span>
      </button>
      <span v-else class="tree-toggle-spacer"></span>
      <SidebarSessionItem
        class="tree-item"
        :session="session"
        :active="session.id === sessionStore.currentId"
        :folders="folders"
        @delete="(id, title) => emit('delete', id, title)"
      />
    </div>
    <template v-if="expanded">
      <SidebarSessionTree
        v-for="child in children"
        :key="child.id"
        :session="child"
        :folders="folders"
        :depth="depth + 1"
        :is-expanded="isExpanded"
        @delete="(id, title) => emit('delete', id, title)"
        @toggle="(id) => emit('toggle', id)"
      />
    </template>
  </div>
</template>

<style scoped>
.tree-row {
  display: flex;
  align-items: center;
  gap: 2px;
}

.tree-item {
  flex: 1;
  min-width: 0;
}

.tree-toggle {
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: #a1a1aa;
  padding: 2px;
  border-radius: 4px;
  cursor: pointer;
  flex-shrink: 0;
}

.tree-toggle:hover {
  background: #e4e4e7;
  color: #18181b;
}

.tree-toggle .msi {
  font-size: 16px;
}

.tree-toggle-spacer {
  width: 20px;
  flex-shrink: 0;
}
</style>
