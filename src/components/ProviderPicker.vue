<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import Select from "primevue/select";
import { PROVIDER_KINDS, providerLabel, useProviderStore } from "../stores/provider";
import { api } from "../api";
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
  /** Só true quando usado no topbar do chat (`ComposerBar.vue`) — esse
   * contexto já tem seu PRÓPRIO ícone de visão (`vision-icon-btn`, Fase E4,
   * com cache em localStorage) que chama o mesmo `api.testVision` por
   * baixo; sem essa flag os dois apareciam lado a lado duplicados (achado
   * testando ao vivo, 2026-08-17). Não tem mais nada a ver com "colapsar" —
   * o modo resumido/recolhido foi removido a pedido do usuário
   * (2026-08-20), o seletor agora fica sempre expandido. */
  hideVisionTest?: boolean;
}>();

const emit = defineEmits<{
  "update:provider": [value: ProviderKind];
  "update:fork": [value: string];
  "update:customProviderId": [value: string];
  "update:model": [value: string];
}>();

const providerStore = useProviderStore();

const providerOptions = computed(() => PROVIDER_KINDS.map((kind) => ({
  kind,
  label: providerLabel(kind),
})));

const forkOptions = computed(() => providerStore.forks.map((f) => ({ id: f.id, label: f.label })));
const customProviderOptions = computed(() => providerStore.customProviders.map((p) => ({ id: p.id, label: p.label })));
// Reusa o slot de `customProviderId` pra guardar qual dos 4 CLIs externos
// está selecionado (mesma convenção de `modelsCacheKey` em stores/provider.ts).
const cliBackendOptions = computed(() =>
  providerStore.cliReadiness.map((b) => ({ id: b.backend, label: b.label, installed: b.installed })),
);

const modelOptions = computed(() => {
  const visible = providerStore.visibleModelsFor(props.provider, props.fork, props.customProviderId);
  // Garante que o modelo já escolhido sempre apareça, mesmo quando ele não
  // está nos favoritos nem nos primeiros N (senão o Select perderia o valor).
  if (props.model && !visible.some((m) => m.id === props.model)) {
    const current = providerStore
      .modelsFor(props.provider, props.fork, props.customProviderId)
      .find((m) => m.id === props.model);
    if (current) return [current, ...visible];
  }
  return visible;
});
const modelsLoading = computed(() => providerStore.modelsLoadingFor(props.provider, props.fork, props.customProviderId));

const currentModelInfo = computed(() => modelOptions.value.find((m) => m.id === props.model));

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
}

function refresh(kind: ProviderKind, forkId: string, customProviderId: string) {
  providerStore.refreshModels(kind, forkId, customProviderId);
  providerStore.loadFavorites(kind, forkId, customProviderId);
}

watch(
  () => [props.provider, props.fork, props.customProviderId] as const,
  ([kind, forkId, customProviderId]) => refresh(kind, forkId, customProviderId),
);

onMounted(() => refresh(props.provider, props.fork, props.customProviderId));

// Só existe pra tela de Nova Sessão (`hideVisionTest` falso/ausente) — o
// topbar do chat já criado (`hideVisionTest=true`, `ComposerBar.vue`) tem
// seu PRÓPRIO ícone de visão (`vision-icon-btn`, Fase E4, com cache em
// localStorage) que chama o mesmo `api.testVision` por baixo. Os dois
// ficavam lado a lado duplicados antes disso — achado testando ao vivo,
// 2026-08-17 (usuário viu "2 ícones de visão" e estranhou, com razão).
const visionStatus = ref<"idle" | "checking" | "yes" | "no" | "error">("idle");
const visionError = ref("");

async function checkVision() {
  if (!props.model) return;
  visionStatus.value = "checking";
  visionError.value = "";
  try {
    const result = await api.testVision(
      props.provider,
      props.provider === "custom" ? props.customProviderId : null,
      props.model,
    );
    visionStatus.value = result ? "yes" : "no";
  } catch (e) {
    visionStatus.value = "error";
    visionError.value = String(e);
  }
}

watch(() => props.model, () => { visionStatus.value = "idle"; });
</script>

<template>
  <div class="picker-row">
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
      :placeholder="$t('providerPicker.connection')"
      class="picker-select fork-select"
      size="small"
    />
    <Select
      v-if="provider === 'cli'"
      :modelValue="customProviderId"
      @update:modelValue="(v) => setCustomProviderId(v as string)"
      :options="cliBackendOptions"
      optionLabel="label"
      optionValue="id"
      :placeholder="$t('providerPicker.cliBackend')"
      class="picker-select fork-select"
      size="small"
    >
      <template #option="{ option }">
        <div class="cli-backend-option">
          <span>{{ option.label }}</span>
          <span
            class="cli-installed-dot"
            :class="{ installed: option.installed }"
            v-tooltip.top="option.installed ? $t('providerPicker.cliInstalled') : $t('providerPicker.cliNotInstalled')"
          />
        </div>
      </template>
    </Select>
    <Select
      :modelValue="model"
      @update:modelValue="(v) => setModel(v as string)"
      :options="modelOptions"
      optionLabel="label"
      optionValue="id"
      :placeholder="modelsLoading ? $t('providerPicker.loading') : $t('providerPicker.model')"
      :loading="modelsLoading"
      class="picker-select model-select"
      size="small"
      filter
    >
      <template #option="{ option }">
        <div class="model-option">
          <span class="model-option-label">{{ option.label }}</span>
          <span v-if="option.supports_vision" class="cap-badge vision" v-tooltip.top="$t('modelBrowser.acceptsImage')">
            <span class="msi">image</span>
          </span>
          <span v-if="option.supports_tools" class="cap-badge tools" v-tooltip.top="$t('modelBrowser.supportsTools')">
            <span class="msi">build</span>
          </span>
          <span v-if="option.supports_audio" class="cap-badge audio" v-tooltip.top="$t('modelBrowser.acceptsAudio')">
            <span class="msi">mic</span>
          </span>
        </div>
      </template>
    </Select>
    <span v-if="currentModelInfo?.supports_vision" class="cap-badge vision" v-tooltip.top="$t('modelBrowser.acceptsImage')">
      <span class="msi">image</span>
    </span>
    <span v-if="currentModelInfo?.supports_tools" class="cap-badge tools" v-tooltip.top="$t('modelBrowser.supportsTools')">
      <span class="msi">build</span>
    </span>
    <span v-if="currentModelInfo?.supports_audio" class="cap-badge audio" v-tooltip.top="$t('modelBrowser.acceptsAudio')">
      <span class="msi">mic</span>
    </span>
    <!-- Some quando `hideVisionTest` — o composer já tem seu próprio ícone
         de visão (ComposerBar.vue::vision-icon-btn, Fase E4); mostrar os
         dois juntos duplicava a mesma checagem lado a lado (achado testando
         ao vivo, 2026-08-17). -->
    <button
      v-if="model && provider !== 'llama_cpp' && provider !== 'cli' && !hideVisionTest"
      class="vision-btn"
      :class="visionStatus"
      :disabled="visionStatus === 'checking'"
      v-tooltip.top="visionStatus === 'yes' ? $t('providerPicker.supportsImages') : visionStatus === 'no' ? $t('providerPicker.noImageSupport') : visionStatus === 'error' ? visionError : $t('providerPicker.testImageSupport')"
      @click="checkVision"
    >
      <span class="msi spin" v-if="visionStatus === 'checking'">progress_activity</span>
      <span v-else-if="visionStatus === 'yes'">👁️</span>
      <span v-else-if="visionStatus === 'no'">🚫</span>
      <span v-else-if="visionStatus === 'error'">⚠️</span>
      <span v-else class="msi">visibility</span>
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
  width: 340px;
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

.vision-btn {
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
  font-size: 14px;
  transition: all 0.15s ease;
}

.vision-btn:hover {
  background: #f4f4f5;
}

.vision-btn.yes {
  background: #ecfdf3;
  border-color: #86efac;
  color: #15803d;
}

.vision-btn.no {
  background: #fef2f2;
  border-color: #fca5a5;
  color: #b91c1c;
}

.vision-btn.error {
  background: #fffbeb;
  border-color: #fcd34d;
  color: #b45309;
}

.vision-btn .msi {
  font-size: 16px;
}

.model-option {
  display: flex;
  align-items: center;
  gap: 7px;
  min-width: 0;
}

.model-option-label {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.cap-badge {
  display: inline-flex;
  align-items: center;
  color: #2563eb;
  flex-shrink: 0;
}

.cap-badge.tools {
  color: #16a34a;
}

.cap-badge.audio {
  color: #9333ea;
}

.cap-badge .msi {
  font-size: 14px;
}

.cli-backend-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
}

.cli-installed-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #d4d4d8;
  flex-shrink: 0;
}

.cli-installed-dot.installed {
  background: #22c55e;
}
</style>
