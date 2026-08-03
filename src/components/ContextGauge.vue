<script setup lang="ts">
import { computed, nextTick, ref } from "vue";
import { useI18n } from "vue-i18n";
import { useSessionStore } from "../stores/session";

const { t, locale } = useI18n();
const sessionStore = useSessionStore();

const usage = computed(() => sessionStore.contextUsage);

const percent = computed(() => Math.min(100, Math.max(0, usage.value?.percent ?? 0)));

const level = computed<"low" | "mid" | "high">(() => {
  if (percent.value >= 85) return "high";
  if (percent.value >= 50) return "mid";
  return "low";
});

function formatTokens(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`;
  return `${n}`;
}

const label = computed(() => {
  if (!usage.value) return "";
  const { used_tokens, context_length, is_estimated_length } = usage.value;
  const approx = is_estimated_length ? "~" : "";
  return `${formatTokens(used_tokens)} / ${approx}${formatTokens(context_length)}`;
});

const tooltip = computed(() => {
  if (!usage.value) return "";
  const base = t("contextGauge.tooltipBase", {
    used: usage.value.used_tokens,
    total: usage.value.context_length,
    percent: percent.value.toFixed(0),
  });
  return usage.value.is_estimated_length
    ? `${base}. ${t("contextGauge.tooltipEstimated")}`
    : `${base}. ${t("contextGauge.tooltipClickToFix")}`;
});

const editing = ref(false);
const inputValue = ref("");
const inputRef = ref<HTMLInputElement | null>(null);
const error = ref("");

async function startEdit() {
  inputValue.value = sessionStore.currentSession?.context_length?.toString() ?? "";
  error.value = "";
  editing.value = true;
  await nextTick();
  inputRef.value?.focus();
  inputRef.value?.select();
}

async function save() {
  // `type="number"` faz o v-model converter automaticamente pra Number
  // (comportamento nativo do Vue 3 pra esse tipo de input) - sem o
  // String(), `.trim()` quebra com "not a function" quando o valor já não
  // é string, erro que o Vue engole silenciosamente no event handler.
  const trimmed = String(inputValue.value ?? "").trim();
  if (!trimmed) {
    await sessionStore.updateContextLength(null);
    editing.value = false;
    return;
  }
  const parsed = Number(trimmed);
  if (!Number.isFinite(parsed) || parsed <= 0) {
    error.value = t("contextGauge.invalidTokens");
    return;
  }
  try {
    await sessionStore.updateContextLength(Math.round(parsed));
    editing.value = false;
  } catch (e) {
    error.value = String(e);
  }
}

function cancel() {
  editing.value = false;
  error.value = "";
}
</script>

<template>
  <div v-if="usage" class="context-gauge-wrap">
    <button
      v-if="!editing"
      class="context-gauge"
      :class="level"
      v-tooltip.top="tooltip"
      @click="startEdit"
    >
      <span class="msi">data_usage</span>
      <span class="gauge-track">
        <span class="gauge-fill" :style="{ width: percent + '%' }" />
      </span>
      <span class="gauge-label">{{ label }}</span>
    </button>
    <div v-else class="context-edit">
      <input
        ref="inputRef"
        v-model="inputValue"
        type="number"
        class="context-edit-input"
        :placeholder="$t('contextGauge.tokensPlaceholder')"
        @keydown.enter="save"
        @keydown.escape="cancel"
      />
      <button class="context-edit-btn" v-tooltip.top="$t('sidebar.save')" @click="save">
        <span class="msi">check</span>
      </button>
      <button class="context-edit-btn" v-tooltip.top="$t('taskList.cancel')" @click="cancel">
        <span class="msi">close</span>
      </button>
    </div>
    <p v-if="error" class="context-edit-error">{{ error }}</p>
    <span
      v-if="!editing && usage.total_requests > 0"
      class="usage-badges"
      v-tooltip.top="$t('contextGauge.usageTooltip', { input: usage.total_prompt_tokens.toLocaleString(locale), output: usage.total_completion_tokens.toLocaleString(locale), requests: usage.total_requests })"
    >
      ↓{{ formatTokens(usage.total_prompt_tokens) }}
      ↑{{ formatTokens(usage.total_completion_tokens) }}
      🔄{{ usage.total_requests }}
    </span>
  </div>
</template>

<style scoped>
.context-gauge-wrap {
  position: relative;
  display: flex;
  align-items: center;
  gap: 6px;
}

.context-gauge {
  display: flex;
  align-items: center;
  gap: 5px;
  font-size: 11px;
  font-weight: 600;
  color: #71717a;
  padding: 4px 8px;
  border: var(--cerne-border);
  border-radius: 999px;
  background: #fafafa;
  cursor: pointer;
  font-family: inherit;
}

.context-gauge:hover {
  background: #f4f4f5;
  border-color: #a1a1aa;
}

.context-gauge .msi {
  font-size: 14px;
}

.gauge-track {
  width: 44px;
  height: 4px;
  border-radius: 999px;
  background: #e4e4e7;
  overflow: hidden;
}

.gauge-fill {
  display: block;
  height: 100%;
  border-radius: 999px;
  background: #52525b;
  transition: width 0.2s ease;
}

.context-gauge.mid .gauge-fill {
  background: #d97706;
}

.context-gauge.high .gauge-fill {
  background: #dc2626;
}

.context-gauge.high {
  color: #dc2626;
}

.gauge-label {
  font-variant-numeric: tabular-nums;
  white-space: nowrap;
}

.context-edit {
  display: flex;
  align-items: center;
  gap: 4px;
}

.context-edit-input {
  width: 110px;
  border: var(--cerne-border);
  border-radius: 999px;
  padding: 4px 10px;
  font-size: 11px;
  font-weight: 600;
  outline: none;
}

.context-edit-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 22px;
  height: 22px;
  border: var(--cerne-border);
  border-radius: 999px;
  background: #ffffff;
  cursor: pointer;
  color: #52525b;
  flex-shrink: 0;
}

.context-edit-btn:hover {
  background: #f4f4f5;
}

.context-edit-btn .msi {
  font-size: 13px;
}

.context-edit-error {
  position: absolute;
  top: 100%;
  right: 0;
  margin-top: 4px;
  font-size: 11px;
  font-weight: 500;
  color: #dc2626;
  white-space: nowrap;
  background: #ffffff;
  padding: 2px 6px;
  border-radius: 6px;
  border: 1px solid #fecaca;
}

.usage-badges {
  font-size: 10px;
  font-weight: 600;
  color: #71717a;
  white-space: nowrap;
  font-variant-numeric: tabular-nums;
  cursor: default;
}
</style>
