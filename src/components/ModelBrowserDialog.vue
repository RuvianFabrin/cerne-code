<script setup lang="ts">
import { computed, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import DataTable from "primevue/datatable";
import Column from "primevue/column";
import { useProviderStore } from "../stores/provider";
import type { ModelInfo, ProviderKind } from "../api";

const { locale } = useI18n();

// Modal de navegação de modelos — tabela com busca, paginação e favoritos
// (estrela). Abre a partir da tela de configurações pra cada provedor/conexão.
// A lista completa vem do store; os favoritos marcados aqui são os mesmos que
// o ProviderPicker usa pra montar o dropdown (só favoritos, ou os primeiros).
const props = defineProps<{
  visible: boolean;
  kind: ProviderKind;
  forkId: string;
  customProviderId: string;
  title: string;
}>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();

const providerStore = useProviderStore();

const search = ref("");
const onlyFavorites = ref(false);

watch(
  () => props.visible,
  (v) => {
    if (v) {
      search.value = "";
      onlyFavorites.value = false;
      providerStore.refreshModels(props.kind, props.forkId, props.customProviderId);
      providerStore.loadFavorites(props.kind, props.forkId, props.customProviderId);
    }
  },
);

const models = computed(() => providerStore.modelsFor(props.kind, props.forkId, props.customProviderId));
const loading = computed(() => providerStore.modelsLoadingFor(props.kind, props.forkId, props.customProviderId));

const filteredModels = computed(() => {
  let list = models.value;
  if (onlyFavorites.value) {
    const favs = new Set(providerStore.favoritesFor(props.kind, props.forkId, props.customProviderId));
    list = list.filter((m) => favs.has(m.id));
  }
  const q = search.value.trim().toLowerCase();
  if (q) {
    list = list.filter((m) =>
      [m.id, m.name, m.description, m.parameter_size].some((f) => f && f.toLowerCase().includes(q)),
    );
  }
  return list;
});

function isFavorite(id: string): boolean {
  return providerStore.isFavorite(props.kind, id, props.forkId, props.customProviderId);
}

function toggleFavorite(id: string) {
  providerStore.toggleFavorite(props.kind, id, props.forkId, props.customProviderId);
}

function displayName(m: ModelInfo): string {
  return m.name && m.name !== m.id ? m.name : m.id;
}

function formatCtx(n?: number | null): string {
  if (n == null) return "—";
  if (n >= 1_000_000) return `${(n / 1_000_000).toLocaleString(locale.value, { maximumFractionDigits: 1 })}M`;
  if (n >= 1000) return `${Math.round(n / 1000)}K`;
  return `${n}`;
}

function formatSize(m: ModelInfo): string {
  const parts: string[] = [];
  if (m.parameter_size) parts.push(m.parameter_size);
  if (m.size_bytes != null) {
    const gb = m.size_bytes / 1024 ** 3;
    parts.push(`${gb.toLocaleString(locale.value, { maximumFractionDigits: 1 })} GB`);
  }
  return parts.length ? parts.join(" · ") : "—";
}

function formatPrice(p?: number | null): string {
  if (p == null) return "—";
  const perMillion = p * 1_000_000;
  return `$${perMillion.toLocaleString(locale.value, { maximumFractionDigits: 2 })}`;
}

function priceLabel(m: ModelInfo): string {
  if (m.price_prompt == null && m.price_completion == null) return "—";
  return `${formatPrice(m.price_prompt)} / ${formatPrice(m.price_completion)}`;
}
</script>

<template>
  <Dialog
    :visible="props.visible"
    @update:visible="(v) => emit('update:visible', v)"
    :header="title"
    modal
    :style="{ width: 'min(860px, 92vw)' }"
    :contentStyle="{ padding: 0 }"
  >
    <div class="browser-toolbar">
      <input v-model="search" class="text-input search-input" :placeholder="$t('modelBrowser.searchPlaceholder')" />
      <label class="fav-filter">
        <input type="checkbox" v-model="onlyFavorites" />
        {{ $t("modelBrowser.onlyFavorites") }}
      </label>
      <span class="count">{{ $t("modelBrowser.modelCount", { count: filteredModels.length }) }}</span>
    </div>

    <DataTable
      :value="filteredModels"
      :loading="loading"
      paginator
      :rows="25"
      :rowsPerPageOptions="[25, 50, 100]"
      size="small"
      stripedRows
      removableSort
      class="model-table"
    >
      <template #empty>
        <p class="empty-hint">{{ loading ? $t("modelBrowser.loadingModels") : $t("modelBrowser.noModelsFound") }}</p>
      </template>
      <Column :sortable="false" style="width: 44px; text-align: center">
        <template #body="{ data }">
          <button
            class="star-btn"
            :class="{ active: isFavorite(data.id) }"
            v-tooltip.top="isFavorite(data.id) ? $t('providerPicker.removeFavorite') : $t('modelBrowser.markFavorite')"
            @click="toggleFavorite(data.id)"
          >
            <span class="msi">{{ isFavorite(data.id) ? "star" : "star_border" }}</span>
          </button>
        </template>
      </Column>
      <Column field="id" :header="$t('providerPicker.model')" sortable style="min-width: 240px">
        <template #body="{ data }">
          <div class="model-cell">
            <span class="model-name">
              {{ displayName(data) }}
              <span v-if="data.supports_vision" class="cap-badge vision" v-tooltip.top="$t('modelBrowser.acceptsImage')">
                <span class="msi">image</span>
              </span>
              <span
                v-else-if="data.vision_hint"
                class="cap-badge vision-hint"
                v-tooltip.top="$t('modelBrowser.visionFamilyHint', { family: data.vision_hint })"
              >
                <span class="msi">image</span>
              </span>
              <span v-if="data.supports_tools" class="cap-badge tools" v-tooltip.top="$t('modelBrowser.supportsTools')">
                <span class="msi">build</span>
              </span>
              <span v-if="data.supports_audio" class="cap-badge audio" v-tooltip.top="$t('modelBrowser.acceptsAudio')">
                <span class="msi">mic</span>
              </span>
            </span>
            <span v-if="data.name && data.name !== data.id" class="model-id">{{ data.id }}</span>
            <span v-if="data.description" class="model-desc">{{ data.description }}</span>
          </div>
        </template>
      </Column>
      <Column field="context_length" :header="$t('modelBrowser.ctx')" sortable style="width: 90px; text-align: right">
        <template #body="{ data }">{{ formatCtx(data.context_length) }}</template>
      </Column>
      <Column field="size_bytes" :header="$t('modelBrowser.size')" sortable style="width: 130px; text-align: right">
        <template #body="{ data }">{{ formatSize(data) }}</template>
      </Column>
      <Column field="price_prompt" :header="$t('modelBrowser.price')" sortable style="width: 150px; text-align: right">
        <template #body="{ data }">{{ priceLabel(data) }}</template>
      </Column>
    </DataTable>
  </Dialog>
</template>

<style scoped>
.browser-toolbar {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 12px 16px;
  border-bottom: var(--cerne-border);
}

.text-input {
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 7px 10px;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  outline: none;
}

.search-input {
  flex: 1;
  min-width: 0;
}

.fav-filter {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  color: #52525b;
  white-space: nowrap;
  cursor: pointer;
}

.count {
  font-size: 12px;
  color: #a1a1aa;
  white-space: nowrap;
}

.empty-hint {
  text-align: center;
  color: #a1a1aa;
  font-size: 13px;
  padding: 24px 0;
}

.model-table {
  font-size: 13px;
}

:deep(.p-datatable-header-cell),
:deep(.p-datatable-column-header-content) {
  font-size: 12px;
}

.star-btn {
  border: none;
  background: transparent;
  cursor: pointer;
  color: #d4d4d8;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 2px;
  border-radius: 6px;
}

.star-btn:hover {
  color: #f59e0b;
}

.star-btn.active {
  color: #f59e0b;
}

.star-btn .msi {
  font-size: 20px;
}

.model-cell {
  display: flex;
  flex-direction: column;
  gap: 1px;
  min-width: 0;
}

.model-name {
  font-weight: 600;
  color: #27272a;
  display: flex;
  align-items: center;
  gap: 5px;
}

.cap-badge {
  display: inline-flex;
  align-items: center;
  color: #2563eb;
}

.cap-badge.vision-hint {
  color: #d97706;
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

.model-id {
  font-size: 11px;
  color: #a1a1aa;
  font-family: ui-monospace, monospace;
}

.model-desc {
  font-size: 11px;
  color: #71717a;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}
</style>
