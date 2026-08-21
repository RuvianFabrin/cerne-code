<script setup lang="ts">
import { useSessionStore } from "../stores/session";

const sessionStore = useSessionStore();

// Fase A5 do roteiro de Agentes/Skills: quando o modelo planeja usar mais de
// um agente/skill no mesmo turno (modo Manual), pergunta uma vez só,
// batelado, em vez de um popup por chamada (ver PermissionCard.vue, que
// continua cuidando de tool calls normais).
const ICONS: Record<string, string> = {
  task: "smart_toy",
  load_skill: "menu_book",
  verify_completion: "fact_check",
};

function respond(approved: boolean) {
  sessionStore.answerAgentsSkillsPlan(approved);
}
</script>

<template>
  <div v-if="sessionStore.pendingAgentsSkillsPlan" class="plan-card">
    <div class="plan-header">
      <span class="msi">smart_toy</span>
      <span class="plan-title">{{ $t("agentsSkillsPlan.title") }}</span>
    </div>
    <ul class="plan-items">
      <li v-for="item in sessionStore.pendingAgentsSkillsPlan.items" :key="item.id">
        <span class="msi plan-item-icon">{{ ICONS[item.tool] ?? "extension" }}</span>
        <span class="plan-item-tool">{{ $t(`toolLabels.${item.tool}`) }}:</span>
        <span class="plan-item-name">{{ item.name }}</span>
      </li>
    </ul>
    <p v-if="sessionStore.pendingAgentsSkillsPlan.parallel" class="plan-parallel-note">
      <span class="msi">bolt</span>
      {{ $t("agentsSkillsPlan.parallelNote") }}
    </p>
    <div class="plan-actions">
      <button class="btn-approve" @click="respond(true)">{{ $t("agentsSkillsPlan.approve") }}</button>
      <button class="btn-deny" @click="respond(false)">{{ $t("agentsSkillsPlan.deny") }}</button>
    </div>
  </div>
</template>

<style scoped>
.plan-card {
  border: 1px solid #bfdbfe;
  border-radius: 12px;
  overflow: hidden;
  margin-bottom: 10px;
  background: #eff6ff;
}

.plan-header {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 12px 6px;
}

.plan-header .msi {
  font-size: 18px;
  color: #2563eb;
}

.plan-title {
  font-size: 13px;
  font-weight: 600;
  color: #18181b;
}

.plan-items {
  list-style: none;
  margin: 0 12px 8px;
  padding: 8px 10px;
  background: #ffffff;
  border: 1px solid #bfdbfe;
  border-radius: 8px;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.plan-items li {
  display: flex;
  align-items: baseline;
  gap: 6px;
  font-size: 12px;
}

.plan-item-icon {
  font-size: 15px;
  color: #2563eb;
  align-self: center;
}

.plan-item-tool {
  font-weight: 600;
  color: #18181b;
  flex-shrink: 0;
}

.plan-item-name {
  color: #3f3f46;
  overflow-wrap: break-word;
}

.plan-parallel-note {
  display: flex;
  align-items: center;
  gap: 5px;
  margin: 0 12px 10px;
  font-size: 11px;
  font-weight: 500;
  color: #b45309;
}

.plan-parallel-note .msi {
  font-size: 14px;
}

.plan-actions {
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
