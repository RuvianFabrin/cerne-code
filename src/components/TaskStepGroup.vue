<script setup lang="ts">
import { ref } from "vue";
import type { TaskItem } from "../api";
import { friendlyStepLabel } from "../taskLabels";

defineProps<{ tasks: TaskItem[] }>();

const expanded = ref<Set<string>>(new Set());

function toggle(id: string) {
  if (expanded.value.has(id)) expanded.value.delete(id);
  else expanded.value.add(id);
  // Set não é reativo por mutação direta o suficiente pro template re-renderizar
  // de forma confiável em todos os casos - reatribuir força a atualização.
  expanded.value = new Set(expanded.value);
}

const statusIcon: Record<string, string> = {
  pending: "schedule",
  running: "progress_activity",
  done: "check_circle",
  failed: "error",
};
</script>

<template>
  <div class="step-group">
    <div v-for="t in tasks" :key="t.id" class="step-row" @click="toggle(t.id)">
      <span class="msi status" :class="t.status">{{ statusIcon[t.status] ?? "schedule" }}</span>
      <span class="step-label">{{ friendlyStepLabel(t.label) }}</span>
      <span class="msi chevron" :class="{ open: expanded.has(t.id) }">chevron_right</span>
    </div>
    <template v-for="t in tasks" :key="`detail-${t.id}`">
      <div v-if="expanded.has(t.id)" class="step-detail">
        <div class="step-detail-label">{{ t.label }}</div>
        <div v-if="t.detail" class="step-detail-body">{{ t.detail }}</div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.step-group {
  display: flex;
  flex-direction: column;
  gap: 2px;
  margin: 6px 0;
  padding: 6px 2px;
}

.step-row {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 2px 4px;
  border-radius: 6px;
  cursor: pointer;
  font-size: 12.5px;
  font-weight: 400;
  color: #71717a;
}

.step-row:hover {
  background: #f4f4f5;
  color: #3f3f46;
}

.step-row .status {
  font-size: 15px;
  color: #a1a1aa;
  flex-shrink: 0;
}

.step-row .status.done {
  color: #16a34a;
}

.step-row .status.failed {
  color: #dc2626;
}

.step-row .status.running {
  color: #3f3f46;
  animation: spin 1s linear infinite;
}

.step-label {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.chevron {
  font-size: 16px;
  color: #d4d4d8;
  flex-shrink: 0;
  transition: transform 0.15s ease;
}

.chevron.open {
  transform: rotate(90deg);
}

.step-detail {
  margin: 0 0 2px 23px;
  padding: 6px 10px;
  background: #fafafa;
  border: var(--cerne-border);
  border-radius: 8px;
}

.step-detail-label {
  font-size: 11px;
  font-weight: 500;
  font-family: ui-monospace, monospace;
  color: #52525b;
  overflow-wrap: break-word;
}

.step-detail-body {
  margin-top: 4px;
  font-size: 11px;
  font-weight: 400;
  color: #71717a;
  white-space: pre-wrap;
  overflow-wrap: break-word;
  max-height: 160px;
  overflow-y: auto;
}

@keyframes spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}
</style>
