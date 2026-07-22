<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import Select from "primevue/select";
import { PROVIDER_LABELS, useProviderStore } from "../stores/provider";
import type { ProviderKind } from "../api";

// Purely prop-driven — the caller decides what "the current selection"
// means (a session being edited, or defaults for a session about to be
// created). This component never writes to global config itself, so it's
// safe to reuse in both places without one clobbering the other.
const props = defineProps<{
  provider: ProviderKind;
  fork: string;
  customProviderId: string;
  model: string | null;
  /** Quando true, mostra só um resumo (provider + modelo) depois de
   * escolhido, com um botão pra expandir de volta e reselecionar — usado no
   * composer de uma sessão já criada, pra não ocupar espaço com um seletor
   * que raramente muda turno a turno. A tela de nova sessão passa `false`
   * (padrão), onde a escolha é o próprio propósito da tela. */
  collapsible?: boolean;
}>();

const emit = defineEmits<{
  "update:provider": [value: ProviderKind];
  "update:fork": [value: string];
  "update:customProviderId": [value: string];
  "update:model": [value: string];
}>();

const providerStore = useProviderStore();
// Só começa colapsado se já tem um modelo escolhido — numa sessão nova ainda
// sem modelo, faz sentido já abrir expandido pra escolher.
const expanded = ref(!props.collapsible || !props.model);

const providerOptions = (Object.keys(PROVIDER_LABELS) as ProviderKind[]).map((kind) => ({
  kind,
  label: PROVIDER_LABELS[kind],
}));

const forkOptions = computed(() => providerStore.forks.map((f) => ({ id: f.id, label: f.label })));
const customProviderOptions = computed(() => providerStore.customProviders.map((p) => ({ id: p.id, label: p.label })));

const modelOptions = computed(() => providerStore.modelsFor(props.provider, props.fork, props.customProviderId));
const modelsLoading = computed(() => providerStore.modelsLoadingFor(props.provider, props.fork, props.customProviderId));

const modelLabel = computed(() => modelOptions.value.find((m) => m.id === props.model)?.label ?? props.model);

// No resumo colapsado, um provider customizado mostra o rótulo da conexão
// escolhida (ex: "Claude") em vez do genérico "Customizado" — mesma clareza
// de "fornecedor + modelo" que os providers embutidos já têm.
const providerSummaryLabel = computed(() => {
  if (props.provider !== "custom") return PROVIDER_LABELS[props.provider];
  return customProviderOptions.value.find((p) => p.id === props.customProviderId)?.label ?? PROVIDER_LABELS.custom;
});

function setProvider(kind: ProviderKind) {
  emit("update:provider", kind);
  refresh(kind, props.fork, props.customProviderId);
}

function setFork(forkId: string) {
  emit("update:fork", forkId);
  refresh(props.provider, forkId, props.customProviderId);
}

function setCustomProviderId(id: string) {
  emit("update:customProviderId", id);
  refresh(props.provider, props.fork, id);
}

function setModel(id: string) {
  emit("update:model", id);
  if (props.collapsible) expanded.value = false;
}

function refresh(kind: ProviderKind, forkId: string, customProviderId: string) {
  providerStore.refreshModels(kind, forkId, customProviderId);
}

watch(
  () => [props.provider, props.fork, props.customProviderId] as const,
  ([kind, forkId, customProviderId]) => refresh(kind, forkId, customProviderId),
);

onMounted(() => refresh(props.provider, props.fork, props.customProviderId));
</script>

<template>
  <button v-if="collapsible && !expanded" class="picker-summary" @click="expanded = true">
    <span class="summary-provider">{{ providerSummaryLabel }}</span>
    <span class="summary-sep">·</span>
    <span class="summary-model">{{ modelLabel ?? "Escolher modelo" }}</span>
    <span class="msi">expand_more</span>
  </button>
  <div v-else class="picker-row">
    <Select
      :modelValue="provider"
      @update:modelValue="setProvider"
      :options="providerOptions"
      optionLabel="label"
      optionValue="kind"
      class="picker-select provider-select"
      size="small"
    />
    <Select
      v-if="provider === 'llama_cpp'"
      :modelValue="fork"
      @update:modelValue="setFork"
      :options="forkOptions"
      optionLabel="label"
      optionValue="id"
      class="picker-select fork-select"
      size="small"
    />
    <Select
      v-if="provider === 'custom'"
      :modelValue="customProviderId"
      @update:modelValue="(v) => setCustomProviderId(v as string)"
      :options="customProviderOptions"
      optionLabel="label"
      optionValue="id"
      placeholder="Conexão"
      class="picker-select fork-select"
      size="small"
    />
    <Select
      :modelValue="model"
      @update:modelValue="(v) => setModel(v as string)"
      :options="modelOptions"
      optionLabel="label"
      optionValue="id"
      :placeholder="modelsLoading ? 'Carregando...' : 'Modelo'"
      :loading="modelsLoading"
      class="picker-select model-select"
      size="small"
      filter
    />
    <button v-if="collapsible && model" class="collapse-btn" v-tooltip.top="'Recolher'" @click="expanded = false">
      <span class="msi">expand_less</span>
    </button>
  </div>
</template>

<style scoped>
.picker-row {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex-wrap: nowrap;
  justify-content: flex-start;
}

.picker-select {
  font-size: 12px;
  font-weight: 500;
  flex-shrink: 1;
  min-width: 0;
}

.provider-select {
  width: 130px;
}

.fork-select {
  width: 220px;
}

.model-select {
  width: 220px;
  flex-shrink: 2;
}

:deep(.p-select) {
  border-radius: 8px;
  box-sizing: border-box;
}

:deep(.p-select-label) {
  padding: 5px 8px;
  font-size: 12px;
  font-weight: 500;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.picker-summary {
  display: flex;
  align-items: center;
  gap: 4px;
  border: var(--cerne-border);
  background: #ffffff;
  border-radius: 8px;
  padding: 5px 8px;
  font-size: 12px;
  font-weight: 500;
  color: #3f3f46;
  cursor: pointer;
  max-width: 260px;
}

.picker-summary .msi {
  font-size: 15px;
  color: #a1a1aa;
  flex-shrink: 0;
}

.summary-provider {
  color: #71717a;
  flex-shrink: 0;
}

.summary-sep {
  color: #d4d4d8;
  flex-shrink: 0;
}

.summary-model {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.collapse-btn {
  border: var(--cerne-border);
  background: #ffffff;
  border-radius: 8px;
  width: 26px;
  height: 26px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: #71717a;
  flex-shrink: 0;
}

.collapse-btn .msi {
  font-size: 16px;
}
</style>
