<script setup lang="ts">
import { computed, ref } from "vue";
import { api, type LlamaForkConfig } from "../api";
import { useLlamaHealth } from "../composables/useLlamaHealth";
import { useProviderStore } from "../stores/provider";
import StatusDot from "./StatusDot.vue";

const props = defineProps<{ fork: LlamaForkConfig }>();
const emit = defineEmits<{ browse: [] }>();

const providerStore = useProviderStore();
const forkId = computed(() => props.fork.id);
const { isUp, isLoading, refresh } = useLlamaHealth(forkId);
const actionError = ref("");
const starting = ref(false);

async function start() {
  actionError.value = "";
  starting.value = true;
  try {
    await api.startLlamaServer(props.fork.id);
    await refresh();
  } catch (e) {
    actionError.value = String(e);
  } finally { starting.value = false; }
}

async function stop() {
  actionError.value = "";
  try { await api.stopLlamaServer(props.fork.id); await refresh(); }
  catch (e) { actionError.value = String(e); }
}

async function remove() {
  await providerStore.removeLlamaFork(props.fork.id);
}
</script>

<template>
  <div class="fork-row">
    <StatusDot :up="isUp" :loading="isLoading || starting" />
    <div class="fork-info">
      <span class="fork-label">{{ fork.label }}</span>
      <span class="fork-path">{{ fork.server_exe }}</span>
    </div>
    <span v-if="actionError" class="fork-error" :title="actionError">{{ actionError }}</span>
    <button class="btn-secondary" @click="emit('browse')">{{ $t("llamaForkRow.models") }}</button>
    <button class="btn-secondary" @click="start" :disabled="starting || isLoading || isUp === true">{{ $t("llamaForkRow.start") }}</button>
    <button class="btn-secondary" @click="stop">{{ $t("llamaForkRow.stop") }}</button>
    <button class="btn-secondary" @click="remove">{{ $t("settings.remove") }}</button>
  </div>
</template>

<style scoped>
.fork-row {
  display: flex;
  align-items: center;
  gap: 8px;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
}

.fork-info {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
}

.fork-label {
  font-size: 13px;
  font-weight: 600;
}

.fork-path {
  font-size: 11px;
  font-weight: 500;
  color: #a1a1aa;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fork-error {
  font-size: 11px;
  font-weight: 600;
  color: #dc2626;
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.btn-secondary {
  border: var(--cerne-border);
  background: #ffffff;
  color: #52525b;
  border-radius: 8px;
  padding: 6px 10px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}
</style>
