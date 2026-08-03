<script setup lang="ts">
import { computed } from "vue";
import type { PendingEdit } from "../api";
import { useSessionStore } from "../stores/session";

const props = defineProps<{ edit: PendingEdit }>();
const sessionStore = useSessionStore();

const lines = computed(() => props.edit.diff.split("\n"));

function lineClass(line: string) {
  if (line.startsWith("+") && !line.startsWith("+++")) return "added";
  if (line.startsWith("-") && !line.startsWith("---")) return "removed";
  if (line.startsWith("@@")) return "hunk";
  return "";
}
</script>

<template>
  <div class="diff-card">
    <div class="diff-header">
      <span class="msi">{{ edit.is_new_file ? "note_add" : "difference" }}</span>
      <span class="diff-path">{{ edit.target_path }}</span>
      <div v-if="!edit.already_applied" class="diff-actions">
        <button class="reject" @click="sessionStore.rejectEdit(edit.id)">{{ $t("diffReview.reject") }}</button>
        <button class="accept" @click="sessionStore.acceptEdit(edit.id)">{{ $t("diffReview.accept") }}</button>
      </div>
      <span v-else class="applied-badge">{{ $t("diffReview.applied") }}</span>
    </div>
    <pre class="diff-body"><span v-for="(line, i) in lines" :key="i" :class="lineClass(line)">{{ line }}</span></pre>
  </div>
</template>

<style scoped>
.diff-card {
  border: var(--cerne-border);
  border-radius: 12px;
  overflow: hidden;
  margin-bottom: 10px;
  background: #ffffff;
}

.diff-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 8px 10px;
  border-bottom: var(--cerne-border);
  background: #fafafa;
}

.diff-header .msi {
  font-size: 16px;
  color: #52525b;
}

.diff-path {
  font-size: 12px;
  font-weight: 600;
  font-family: ui-monospace, monospace;
  color: #18181b;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.diff-actions {
  display: flex;
  gap: 6px;
}

.diff-actions button {
  border-radius: 6px;
  font-size: 12px;
  font-weight: 600;
  padding: 4px 10px;
  cursor: pointer;
}

.accept {
  border: none;
  background: #18181b;
  color: #ffffff;
}

.reject {
  border: var(--cerne-border);
  background: #ffffff;
  color: #52525b;
}

.diff-body {
  margin: 0;
  padding: 8px 10px;
  font-size: 12px;
  font-family: ui-monospace, monospace;
  max-height: 320px;
  overflow: auto;
  white-space: pre-wrap;
}

.diff-body span {
  display: block;
}

.diff-body .added {
  background: #ecfdf3;
  color: #15803d;
}

.diff-body .removed {
  background: #fef2f2;
  color: #b91c1c;
}

.diff-body .hunk {
  color: #a1a1aa;
}

.applied-badge {
  font-size: 11px;
  font-weight: 600;
  color: #16a34a;
  background: #ecfdf3;
  border-radius: 4px;
  padding: 2px 8px;
}
</style>
