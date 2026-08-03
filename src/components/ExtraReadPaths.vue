<script setup lang="ts">
import { ref } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Popover from "primevue/popover";
import { useSessionStore } from "../stores/session";

const sessionStore = useSessionStore();
const popoverRef = ref();

function toggle(event: Event) {
  popoverRef.value?.toggle(event);
}

async function addFolder() {
  const selected = await open({ directory: true, multiple: false });
  if (typeof selected !== "string") return;
  const current = sessionStore.currentSession?.extra_read_paths ?? [];
  if (current.includes(selected)) return;
  await sessionStore.updateExtraReadPaths([...current, selected]);
}

async function removeFolder(path: string) {
  const current = sessionStore.currentSession?.extra_read_paths ?? [];
  await sessionStore.updateExtraReadPaths(current.filter((p) => p !== path));
}

function folderName(path: string) {
  return path.split(/[/\\]/).filter(Boolean).pop() ?? path;
}
</script>

<template>
  <button
    class="extra-paths-btn"
    :class="{ active: (sessionStore.currentSession?.extra_read_paths.length ?? 0) > 0 }"
    v-tooltip.top="$t('extraReadPaths.tooltip')"
    @click="toggle"
  >
    <span class="msi">folder_open</span>
    <span v-if="sessionStore.currentSession?.extra_read_paths.length" class="count-badge">
      {{ sessionStore.currentSession.extra_read_paths.length }}
    </span>
  </button>

  <Popover ref="popoverRef">
    <div class="extra-paths-panel">
      <div class="panel-header">
        <span class="panel-title">{{ $t("extraReadPaths.title") }}</span>
        <span class="panel-hint">{{ $t("extraReadPaths.hint") }}</span>
      </div>

      <ul v-if="sessionStore.currentSession?.extra_read_paths.length" class="path-list">
        <li v-for="path in sessionStore.currentSession.extra_read_paths" :key="path" class="path-row" v-tooltip.top="path">
          <span class="msi path-icon">folder</span>
          <span class="path-name">{{ folderName(path) }}</span>
          <button class="remove-btn" v-tooltip.top="$t('settings.remove')" @click="removeFolder(path)">
            <span class="msi">close</span>
          </button>
        </li>
      </ul>
      <div v-else class="empty-state">{{ $t("extraReadPaths.noneConfigured") }}</div>

      <button class="add-btn" @click="addFolder">
        <span class="msi">add</span>
        {{ $t("extraReadPaths.addFolder") }}
      </button>
    </div>
  </Popover>
</template>

<style scoped>
.extra-paths-btn {
  position: relative;
  border: var(--cerne-border);
  background: #ffffff;
  border-radius: 8px;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: #52525b;
  flex-shrink: 0;
}

.extra-paths-btn.active {
  color: #18181b;
  border-color: #a1a1aa;
}

.extra-paths-btn .msi {
  font-size: 16px;
}

.count-badge {
  position: absolute;
  top: -4px;
  right: -4px;
  background: #18181b;
  color: #ffffff;
  font-size: 9px;
  font-weight: 700;
  line-height: 1;
  border-radius: 999px;
  padding: 2px 4px;
  min-width: 12px;
  text-align: center;
}

.extra-paths-panel {
  width: 280px;
  display: flex;
  flex-direction: column;
  gap: 10px;
  padding: 4px 2px;
}

.panel-header {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.panel-title {
  font-size: 13px;
  font-weight: 600;
  color: #18181b;
}

.panel-hint {
  font-size: 11px;
  color: #71717a;
  line-height: 1.4;
}

.path-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: 4px;
  max-height: 180px;
  overflow-y: auto;
}

.path-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 5px 6px;
  border-radius: 6px;
  background: #f4f4f5;
}

.path-icon {
  font-size: 14px;
  color: #71717a;
  flex-shrink: 0;
}

.path-name {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 12px;
  font-weight: 500;
  color: #3f3f46;
}

.remove-btn {
  border: none;
  background: transparent;
  color: #a1a1aa;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  opacity: 0.6;
  border-radius: 4px;
}

.remove-btn:hover {
  opacity: 1;
  color: #ef4444;
  background: #fee2e2;
}

.remove-btn .msi {
  font-size: 14px;
}

.empty-state {
  font-size: 12px;
  color: #a1a1aa;
  padding: 6px 2px;
}

.add-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 6px;
  border: var(--cerne-border);
  background: #ffffff;
  border-radius: 8px;
  padding: 7px 10px;
  font-size: 12px;
  font-weight: 600;
  color: #18181b;
  cursor: pointer;
}

.add-btn .msi {
  font-size: 15px;
}
</style>
