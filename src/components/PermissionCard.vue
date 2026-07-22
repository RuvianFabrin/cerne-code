<script setup lang="ts">
import { computed } from "vue";
import { useSessionStore } from "../stores/session";

const sessionStore = useSessionStore();

// Argumentos chegam como JSON stringificado — mostra formatado quando dá,
// cru como veio senão (não deveria travar a UI por causa disso).
const formattedArgs = computed(() => {
  const raw = sessionStore.pendingPermission?.args ?? "";
  try {
    return JSON.stringify(JSON.parse(raw), null, 2);
  } catch {
    return raw;
  }
});

function respond(approved: boolean) {
  sessionStore.answerPermission(approved);
}
</script>

<template>
  <div v-if="sessionStore.pendingPermission" class="permission-card">
    <div class="permission-header">
      <span class="msi">shield</span>
      <span class="permission-title">
        Modo Manual: aprovar <code>{{ sessionStore.pendingPermission.tool }}</code>?
      </span>
    </div>
    <pre v-if="formattedArgs" class="permission-args">{{ formattedArgs }}</pre>
    <div class="permission-actions">
      <button class="btn-approve" @click="respond(true)">Aprovar</button>
      <button class="btn-deny" @click="respond(false)">Recusar</button>
    </div>
  </div>
</template>

<style scoped>
.permission-card {
  border: 1px solid #bfdbfe;
  border-radius: 12px;
  overflow: hidden;
  margin-bottom: 10px;
  background: #eff6ff;
}

.permission-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px 6px;
}

.permission-header .msi {
  font-size: 18px;
  color: #2563eb;
}

.permission-title {
  font-size: 13px;
  font-weight: 600;
  color: #18181b;
}

.permission-title code {
  font-family: ui-monospace, monospace;
  background: #dbeafe;
  border-radius: 4px;
  padding: 1px 5px;
}

.permission-args {
  margin: 0 12px 8px;
  padding: 8px 10px;
  background: #ffffff;
  border: 1px solid #bfdbfe;
  border-radius: 8px;
  font-size: 11px;
  font-family: ui-monospace, monospace;
  color: #3f3f46;
  max-height: 160px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
}

.permission-actions {
  display: flex;
  gap: 8px;
  padding: 0 12px 12px;
}

.btn-approve {
  border: none;
  background: #18181b;
  color: #ffffff;
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.btn-deny {
  border: 1px solid #bfdbfe;
  background: #ffffff;
  color: #3f3f46;
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.btn-deny:hover {
  background: #fee2e2;
  border-color: #fecaca;
  color: #b91c1c;
}
</style>
