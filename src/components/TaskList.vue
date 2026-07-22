<script setup lang="ts">
import { useSessionStore } from "../stores/session";

const sessionStore = useSessionStore();

const statusIcon: Record<string, string> = {
  pending: "schedule",
  running: "progress_activity",
  done: "check_circle",
  failed: "error",
};
</script>

<template>
  <div v-if="sessionStore.tasks.length > 0" class="task-panel">
    <div class="task-panel-title">
      <span class="msi">checklist</span>
      Tarefas desta sessão
      <button v-if="sessionStore.status !== 'idle'" class="cancel-btn" v-tooltip.top="'Cancelar execução'" @click="sessionStore.cancelTurn()">
        <span class="msi">stop_circle</span>
        Cancelar
      </button>
    </div>
    <div class="task-item" v-for="t in sessionStore.tasks" :key="t.id">
      <span class="msi status" :class="t.status">{{ statusIcon[t.status] ?? "schedule" }}</span>
      <div class="task-body">
        <span class="task-label">{{ t.label }}</span>
        <span v-if="t.detail" class="task-detail">{{ t.detail }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.task-panel {
  border: var(--cerne-border);
  border-radius: 12px;
  padding: 10px 12px;
  margin-bottom: 10px;
  background: #fafafa;
}

.task-panel-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  color: #52525b;
  margin-bottom: 6px;
}

.cancel-btn {
  display: flex;
  align-items: center;
  gap: 3px;
  margin-left: auto;
  border: 1px solid #fecaca;
  background: #ffffff;
  color: #b91c1c;
  border-radius: 6px;
  padding: 3px 7px;
  font-size: 11px;
  font-weight: 600;
  cursor: pointer;
}

.cancel-btn:hover {
  background: #fee2e2;
}

.cancel-btn .msi {
  font-size: 14px;
}

.task-panel-title .msi {
  font-size: 15px;
}

.task-item {
  display: flex;
  gap: 8px;
  padding: 4px 0;
  align-items: flex-start;
}

.task-item .status {
  font-size: 16px;
  margin-top: 1px;
  color: #a1a1aa;
}

.task-item .status.done {
  color: #16a34a;
}

.task-item .status.failed {
  color: #dc2626;
}

.task-item .status.running {
  color: #3f3f46;
}

.task-body {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.task-label {
  font-size: 12px;
  font-weight: 600;
  color: #18181b;
  font-family: ui-monospace, monospace;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.task-detail {
  font-size: 11px;
  font-weight: 500;
  color: #71717a;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
