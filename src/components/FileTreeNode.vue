<script setup lang="ts">
import { ref } from "vue";
import { api, type DirEntryInfo } from "../api";

// Nó recursivo da árvore do FileBrowser.vue (Fase D2) — carrega os filhos
// só quando expandido (lazy), pra não travar em pastas gigantes tipo
// node_modules que o usuário nunca chega a abrir.
const props = defineProps<{ entry: DirEntryInfo; depth: number }>();
const emit = defineEmits<{ pick: [path: string] }>();

const expanded = ref(false);
const loaded = ref(false);
const loading = ref(false);
const children = ref<DirEntryInfo[]>([]);
const error = ref("");

async function toggle() {
  if (!props.entry.is_dir) {
    emit("pick", props.entry.path);
    return;
  }
  expanded.value = !expanded.value;
  if (expanded.value && !loaded.value) {
    loading.value = true;
    error.value = "";
    try {
      children.value = await api.listDirEntries(props.entry.path);
      loaded.value = true;
    } catch (e) {
      error.value = String(e);
    } finally {
      loading.value = false;
    }
  }
}

function pickFolder() {
  emit("pick", props.entry.path);
}
</script>

<template>
  <div class="ftn-row" :style="{ paddingLeft: `${depth * 16}px` }">
    <button class="ftn-toggle" @click="toggle">
      <span v-if="entry.is_dir" class="msi ftn-chevron" :class="{ open: expanded }">chevron_right</span>
      <span v-else class="ftn-chevron-spacer"></span>
      <span class="msi ftn-icon">{{ entry.is_dir ? "folder" : "description" }}</span>
      <span class="ftn-name">{{ entry.name }}</span>
    </button>
    <button v-if="entry.is_dir" class="ftn-pick" @click.stop="pickFolder" v-tooltip.top="$t('fileBrowser.insertFolder')">
      <span class="msi">add_link</span>
    </button>
  </div>
  <template v-if="entry.is_dir && expanded">
    <p v-if="loading" class="ftn-hint" :style="{ paddingLeft: `${(depth + 1) * 16}px` }">{{ $t("fileBrowser.loading") }}</p>
    <p v-else-if="error" class="ftn-hint ftn-error" :style="{ paddingLeft: `${(depth + 1) * 16}px` }">{{ error }}</p>
    <p v-else-if="children.length === 0" class="ftn-hint" :style="{ paddingLeft: `${(depth + 1) * 16}px` }">{{ $t("fileBrowser.empty") }}</p>
    <FileTreeNode
      v-for="child in children"
      :key="child.path"
      :entry="child"
      :depth="depth + 1"
      @pick="(p) => emit('pick', p)"
    />
  </template>
</template>

<style scoped>
.ftn-row {
  display: flex;
  align-items: center;
  gap: 2px;
}

.ftn-toggle {
  flex: 1;
  display: flex;
  align-items: center;
  gap: 4px;
  border: none;
  background: transparent;
  padding: 3px 4px;
  font-size: 12.5px;
  color: #3f3f46;
  cursor: pointer;
  border-radius: 6px;
  text-align: left;
  min-width: 0;
}

.ftn-toggle:hover {
  background: #f4f4f5;
}

.ftn-chevron {
  font-size: 16px;
  color: #a1a1aa;
  flex-shrink: 0;
  transition: transform 0.12s ease;
}

.ftn-chevron.open {
  transform: rotate(90deg);
}

.ftn-chevron-spacer {
  width: 16px;
  flex-shrink: 0;
}

.ftn-icon {
  font-size: 15px;
  color: #71717a;
  flex-shrink: 0;
}

.ftn-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.ftn-pick {
  flex-shrink: 0;
  border: none;
  background: transparent;
  color: #a1a1aa;
  cursor: pointer;
  display: flex;
  align-items: center;
  padding: 3px;
  border-radius: 4px;
  opacity: 0;
}

.ftn-row:hover .ftn-pick {
  opacity: 1;
}

.ftn-pick:hover {
  background: #e4e4e7;
  color: #18181b;
}

.ftn-hint {
  margin: 2px 0;
  font-size: 11px;
  color: #a1a1aa;
}

.ftn-error {
  color: #dc2626;
}
</style>
