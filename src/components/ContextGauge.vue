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
  const { used_tokens, context_length, is_estimated_length, is_estimated_usage } = usage.value;
  // O `~` marca o que é ESTIMATIVA, e os dois lados têm motivo próprio pra ser:
  // - `used_tokens`: só estimado antes da primeira resposta da sessão (depois
  //   disso é o `prompt_tokens` real que o provider devolveu).
  // - `context_length`: a janela do modelo quando o provider não informou.
  const usedPrefix = is_estimated_usage ? "~" : "";
  const lenPrefix = is_estimated_length ? "~" : "";
  return `${usedPrefix}${formatTokens(used_tokens)} / ${lenPrefix}${formatTokens(context_length)}`;
});

const tooltip = computed(() => {
  if (!usage.value) return "";
  const u = usage.value;
  const base = t("contextGauge.tooltipBase", {
    used: u.used_tokens,
    total: u.context_length,
    percent: percent.value.toFixed(0),
  });
  // Diz de onde veio o número — é a diferença entre "o provider contou" e
  // "nós chutamos", e o usuário merece saber qual dos dois está vendo.
  const origem = u.is_estimated_usage
    ? t("contextGauge.sourceEstimated")
    : t("contextGauge.sourceReal");
  const janela = u.is_estimated_length ? ` ${t("contextGauge.tooltipEstimated")}` : "";
  const corrigir = u.is_estimated_length ? ` ${t("contextGauge.tooltipClickToFix")}` : "";
  return `${base}. ${origem}${janela}${corrigir}`;
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

const compacting = ref(false);
const compactDone = ref(false);

async function compactNow() {
  if (compacting.value) return;
  compacting.value = true;
  try {
    const didCompact = await sessionStore.compactNow();
    if (didCompact) {
      compactDone.value = true;
      setTimeout(() => (compactDone.value = false), 2000);
    }
  } catch (e) {
    error.value = String(e);
  } finally {
    compacting.value = false;
  }
}
</script>

<template>
  <div v-if="usage" class="context-gauge-wrap">
    <button
      v-if="!editing"
      class="context-gauge"
      :class="level"
      :aria-label="label"
      v-tooltip.top="tooltip"
      @click="startEdit"
    >
      <span class="gauge-track">
        <span class="gauge-fill" :style="{ width: percent + '%' }" />
      </span>
    </button>
    <button
      v-if="!editing && sessionStore.currentSession?.provider !== 'cli'"
      class="compact-now-btn"
      :class="{ done: compactDone }"
      :disabled="compacting"
      v-tooltip.top="$t('contextGauge.compactNowTooltip')"
      @click="compactNow"
    >
      <span class="msi" :class="{ spin: compacting }">{{ compactDone ? "check" : compacting ? "progress_activity" : "compress" }}</span>
    </button>
    <div v-else-if="editing" class="context-edit">
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
  padding: 4px;
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

.gauge-track {
  width: 64px;
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

.compact-now-btn {
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

.compact-now-btn:hover:not(:disabled) {
  background: #f4f4f5;
}

.compact-now-btn:disabled {
  cursor: default;
}

.compact-now-btn.done {
  color: #16a34a;
  border-color: #86efac;
}

.compact-now-btn .msi {
  font-size: 13px;
}

.compact-now-btn .msi.spin {
  animation: compact-spin 1.5s linear infinite;
}

@keyframes compact-spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
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
