<script setup lang="ts">
import type { TodoItem } from "../api";

defineProps<{ todos: TodoItem[] }>();

const statusIcon: Record<string, string> = {
  pending: "radio_button_unchecked",
  in_progress: "progress_activity",
  completed: "check_circle",
};
</script>

<template>
  <div class="todo-card">
    <div class="todo-title">
      <span class="msi">checklist</span>
      TodoList
    </div>
    <div v-for="(t, i) in todos" :key="i" class="todo-item" :class="t.status">
      <span class="msi status" :class="t.status">{{ statusIcon[t.status] ?? "radio_button_unchecked" }}</span>
      <span class="todo-content">{{ t.content }}</span>
    </div>
  </div>
</template>

<style scoped>
.todo-card {
  margin: 8px 0;
  padding: 10px 14px;
  border-radius: 10px;
  background: #f9fafb;
  border: 1px solid #e5e7eb;
}

.todo-title {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 13px;
  font-weight: 600;
  color: #374151;
  margin-bottom: 6px;
}

.todo-title .msi {
  font-size: 16px;
}

.todo-item {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 3px 0;
  font-size: 13px;
  font-weight: 400;
  color: #6b7280;
}

.todo-item.completed {
  color: #9ca3af;
  text-decoration: line-through;
}

.todo-item.in_progress {
  color: #18181b;
  font-weight: 500;
}

.todo-item .status {
  font-size: 16px;
  flex-shrink: 0;
}

.todo-item .status.pending {
  color: #d1d5db;
}

.todo-item .status.in_progress {
  color: #3b82f6;
  animation: spin 1.5s linear infinite;
}

.todo-item .status.completed {
  color: #22c55e;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
