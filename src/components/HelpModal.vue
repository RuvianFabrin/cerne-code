<script setup lang="ts">
import { computed, ref } from "vue";
import Dialog from "primevue/dialog";
import MarkdownContent from "./MarkdownContent.vue";
import helpContent from "../content/help.md?raw";
import { READY_PROMPTS, type ReadyPrompt } from "../content/prompts";
import { useSessionStore } from "../stores/session";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();

const sessionStore = useSessionStore();
const search = ref("");
const expandedId = ref<string | null>(null);
const showCatalog = ref(false);

const hasProject = computed(() => !!sessionStore.currentSession?.project_root);

const filtered = computed(() => {
  const q = search.value.toLowerCase().trim();
  return READY_PROMPTS.filter((p) => {
    if (p.scope === "code" && !hasProject.value) return false;
    if (p.scope === "chat" && hasProject.value) return false;
    if (!q) return true;
    return (
      p.title.toLowerCase().includes(q) ||
      p.preview.toLowerCase().includes(q) ||
      p.toolsLabel.toLowerCase().includes(q) ||
      p.full.toLowerCase().includes(q)
    );
  });
});

function toggleExpand(id: string) {
  expandedId.value = expandedId.value === id ? null : id;
}

function usePrompt(prompt: ReadyPrompt) {
  sessionStore.setDraft(prompt.full);
  emit("update:visible", false);
}
</script>

<template>
  <Dialog
    :visible="props.visible"
    @update:visible="(v) => emit('update:visible', v)"
    header="Ajuda"
    modal
    :style="{ width: '720px' }"
  >
    <div class="help-body">
      <div class="help-search">
        <span class="msi">search</span>
        <input v-model="search" placeholder="Buscar prompts..." />
      </div>

      <p class="help-hint">
        Ao clicar em <strong>Usar</strong>, o prompt será colado no seu chat — edite à vontade antes de enviar.
      </p>

      <div class="prompt-list">
        <div v-for="p in filtered" :key="p.id" class="prompt-card">
          <div class="prompt-header">
            <span class="prompt-title">{{ p.title }}</span>
            <span class="prompt-scope">{{ p.scope === "code" ? "⌨️ code" : p.scope === "chat" ? "💬 chat" : "🔀 ambos" }}</span>
          </div>
          <div class="prompt-tools">{{ p.toolsLabel }}</div>
          <p class="prompt-preview">{{ p.preview }}</p>
          <div v-if="expandedId === p.id" class="prompt-full">{{ p.full }}</div>
          <div class="prompt-actions">
            <button class="prompt-eye" v-tooltip.top="'Ver prompt completo'" @click="toggleExpand(p.id)">
              <span class="msi">{{ expandedId === p.id ? "visibility_off" : "visibility" }}</span>
            </button>
            <button class="prompt-use" @click="usePrompt(p)">Usar</button>
          </div>
        </div>
        <p v-if="filtered.length === 0" class="prompt-empty">Nenhum prompt encontrado.</p>
      </div>

      <div class="catalog-section">
        <button class="catalog-toggle" @click="showCatalog = !showCatalog">
          <span class="msi">{{ showCatalog ? "expand_less" : "expand_more" }}</span>
          Catálogo de ferramentas
        </button>
        <div v-if="showCatalog" class="catalog-body">
          <MarkdownContent :content="helpContent" />
        </div>
      </div>
    </div>
  </Dialog>
</template>

<style scoped>
.help-body {
  max-height: 70vh;
  overflow-y: auto;
  padding-right: 4px;
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.help-search {
  display: flex;
  align-items: center;
  gap: 8px;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
  color: #71717a;
}

.help-search input {
  border: none;
  outline: none;
  font-size: 13px;
  font-weight: 500;
  flex: 1;
  color: #18181b;
  background: transparent;
}

.help-search .msi {
  font-size: 18px;
}

.help-hint {
  font-size: 12px;
  color: #71717a;
  margin: 0;
  line-height: 1.5;
}

.prompt-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.prompt-card {
  border: var(--cerne-border);
  border-radius: 10px;
  padding: 12px 14px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.prompt-card:hover {
  border-color: #a1a1aa;
}

.prompt-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}

.prompt-title {
  font-size: 13px;
  font-weight: 600;
  color: #18181b;
}

.prompt-scope {
  font-size: 11px;
  color: #a1a1aa;
  white-space: nowrap;
}

.prompt-tools {
  font-size: 11px;
  color: #6366f1;
  font-weight: 500;
}

.prompt-preview {
  font-size: 12px;
  color: #52525b;
  margin: 0;
  line-height: 1.5;
}

.prompt-full {
  font-size: 12px;
  color: #3f3f46;
  background: #f4f4f5;
  border-radius: 6px;
  padding: 8px 10px;
  white-space: pre-wrap;
  line-height: 1.5;
  font-family: "SFMono-Regular", Consolas, "Liberation Mono", Menlo, monospace;
}

.prompt-actions {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 6px;
}

.prompt-eye {
  border: none;
  background: transparent;
  color: #a1a1aa;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 4px;
  border-radius: 6px;
}

.prompt-eye:hover {
  background: #f4f4f5;
  color: #52525b;
}

.prompt-eye .msi {
  font-size: 16px;
}

.prompt-use {
  border: none;
  background: #18181b;
  color: #ffffff;
  font-size: 12px;
  font-weight: 600;
  padding: 5px 14px;
  border-radius: 6px;
  cursor: pointer;
  font-family: inherit;
}

.prompt-use:hover {
  background: #3f3f46;
}

.prompt-empty {
  font-size: 12px;
  color: #a1a1aa;
  text-align: center;
  padding: 16px;
}

.catalog-section {
  border-top: var(--cerne-border);
  padding-top: 8px;
}

.catalog-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  border: none;
  background: transparent;
  color: #52525b;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  padding: 6px 4px;
  font-family: inherit;
}

.catalog-toggle:hover {
  color: #18181b;
}

.catalog-toggle .msi {
  font-size: 18px;
}

.catalog-body {
  padding-top: 8px;
}
</style>
