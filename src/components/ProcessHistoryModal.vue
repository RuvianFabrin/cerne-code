<script setup lang="ts">
import { computed, onMounted, onUnmounted } from "vue";
import { useI18n } from "vue-i18n";
import type { BackgroundJobInfo } from "../api";
import { processToMarkdown, type ProcessLabels } from "../processMarkdown";
import MarkdownContent from "./MarkdownContent.vue";

const props = defineProps<{ job: BackgroundJobInfo }>();
const emit = defineEmits<{ close: [] }>();
const { t, locale } = useI18n();

// Recalcula a cada atualizacao do processo, entao o modal acompanha a saida ao vivo.
const markdown = computed(() => {
  const labels: ProcessLabels = {
    title: t("processModal.title"),
    status: t("processModal.status"),
    started: t("processModal.started"),
    command: t("processModal.command"),
    output: t("processModal.output"),
    noOutput: t("processModal.noOutput"),
    running: t("processModal.running"),
    exited: t("processModal.exited"),
    unknown: t("processModal.unknown"),
  };
  return processToMarkdown(props.job, labels, String(locale.value));
});

function onKey(event: KeyboardEvent) {
  if (event.key === "Escape") emit("close");
}

onMounted(() => window.addEventListener("keydown", onKey));
onUnmounted(() => window.removeEventListener("keydown", onKey));
</script>

<template>
  <div class="pm-backdrop" @click.self="emit('close')">
    <div class="pm-dialog" role="dialog" aria-modal="true" :aria-label="t('processModal.title')">
      <div class="pm-head">
        <span class="pm-title">{{ t("processModal.title") }}</span>
        <button class="pm-close" :aria-label="t('processModal.close')" @click="emit('close')">
          <span class="msi">close</span>
        </button>
      </div>
      <div class="pm-body">
        <MarkdownContent :content="markdown" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.pm-backdrop {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 16px;
  background: rgba(0, 0, 0, 0.35);
}

.pm-dialog {
  width: min(820px, 100%);
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  border: 1px solid var(--border, #d0d7de);
  border-radius: 12px;
  background: var(--surface-2, #fff);
  color: inherit;
}

.pm-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px 14px;
  border-bottom: 1px solid var(--border, #d0d7de);
}

.pm-title {
  font-weight: 600;
}

.pm-close {
  border: 0;
  background: transparent;
  cursor: pointer;
  color: inherit;
}

.pm-body {
  overflow: auto;
  padding: 12px 16px;
}
</style>
