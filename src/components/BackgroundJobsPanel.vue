<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import type { UnlistenFn } from "@tauri-apps/api/event";
import Dialog from "primevue/dialog";
import { api, onBackgroundOutput, type BackgroundJobInfo } from "../api";
import { useSessionStore } from "../stores/session";

const { t } = useI18n();
const sessionStore = useSessionStore();

const jobs = ref<BackgroundJobInfo[]>([]);
const panelOpen = ref(false);
const detailJob = ref<BackgroundJobInfo | null>(null);
let unlisten: UnlistenFn | null = null;
let poll: ReturnType<typeof setInterval> | null = null;

async function refresh() {
  try {
    const all = await api.listBackgroundJobs();
    jobs.value = all.filter((j) => j.session_id === sessionStore.currentId);
  } catch {
    jobs.value = [];
  }
  syncPolling();
}

// Fallback por poll: o push (agent:background_output) só dispara quando
// sai uma linha nova — não cobre o job terminar (status running→exited)
// sem imprimir mais nada depois. Mas só precisa rodar enquanto tiver job
// `running` de verdade: senão ficava chamando `list_background_jobs` pra
// sempre (achado testando ao vivo, 2026-08-19, via aba Rede do DevTools) —
// cada `refresh()` liga/desliga o timer sozinho.
function syncPolling() {
  const active = jobs.value.some((j) => j.status.kind === "running");
  if (active && !poll) {
    poll = setInterval(refresh, 4000);
  } else if (!active && poll) {
    clearInterval(poll);
    poll = null;
  }
}

onMounted(async () => {
  await refresh();
  unlisten = await onBackgroundOutput((e) => {
    const job = jobs.value.find((j) => j.id === e.id);
    if (job) job.output = e.output;
    if (detailJob.value?.id === e.id) detailJob.value.output = e.output;
  });
});

onUnmounted(() => {
  if (unlisten) unlisten();
  if (poll) clearInterval(poll);
});
watch(() => sessionStore.currentId, refresh);

const runningCount = computed(() => jobs.value.filter((j) => j.status.kind === "running").length);

function statusLabel(status: BackgroundJobInfo["status"]): string {
  if (status.kind === "running") return t("backgroundJobs.running");
  if (status.kind === "exited") return t("backgroundJobs.exited", { code: status.code ?? "?" });
  return t("backgroundJobs.unknown");
}

async function stop(job: BackgroundJobInfo) {
  await api.stopBackgroundJob(job.id).catch(() => {});
  await refresh();
  if (detailJob.value?.id === job.id) detailJob.value = null;
}
</script>

<template>
  <div v-if="jobs.length > 0" class="bg-jobs">
    <button class="bg-jobs-toggle" @click="panelOpen = !panelOpen">
      <span class="msi">terminal</span>
      {{ $t("backgroundJobs.count", { count: jobs.length, running: runningCount }) }}
      <span class="msi bg-jobs-chevron" :class="{ open: panelOpen }">expand_more</span>
    </button>
    <div v-if="panelOpen" class="bg-jobs-list">
      <div v-for="job in jobs" :key="job.id" class="bg-job-row" @click="detailJob = job">
        <span class="msi bg-job-status" :class="job.status.kind">
          {{ job.status.kind === "running" ? "progress_activity" : "check_circle" }}
        </span>
        <span class="bg-job-command">{{ job.command }}</span>
        <span class="bg-job-state">{{ statusLabel(job.status) }}</span>
        <button
          v-if="job.status.kind === 'running'"
          class="bg-job-stop"
          v-tooltip.top="$t('backgroundJobs.stop')"
          @click.stop="stop(job)"
        >
          <span class="msi">stop</span>
        </button>
      </div>
    </div>
  </div>

  <Dialog
    :visible="!!detailJob"
    @update:visible="(v) => { if (!v) detailJob = null; }"
    :header="detailJob?.command"
    modal
    :style="{ width: '640px', maxWidth: '92vw' }"
  >
    <pre class="bg-job-output">{{ detailJob?.output || $t("backgroundJobs.noOutput") }}</pre>
    <template #footer>
      <button v-if="detailJob?.status.kind === 'running'" class="btn-secondary" @click="detailJob && stop(detailJob)">
        {{ $t("backgroundJobs.stop") }}
      </button>
    </template>
  </Dialog>
</template>

<style scoped>
.bg-jobs {
  margin: 0 0 6px;
  min-width: 0;
  max-width: 100%;
}

.bg-jobs-toggle {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  max-width: 100%;
  border: var(--cerne-border);
  background: #fafafa;
  border-radius: 8px;
  padding: 5px 10px;
  font-size: 12px;
  font-weight: 500;
  color: #52525b;
  cursor: pointer;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bg-jobs-toggle .msi {
  font-size: 15px;
}

.bg-jobs-chevron {
  transition: transform 0.15s ease;
  font-size: 16px !important;
  color: #a1a1aa;
}

.bg-jobs-chevron.open {
  transform: rotate(180deg);
}

.bg-jobs-list {
  margin-top: 4px;
  border: var(--cerne-border);
  border-radius: 8px;
  max-width: 100%;
  max-height: 280px;
  overflow-y: auto;
  overflow-x: hidden;
}

.bg-job-row {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 10px;
  font-size: 12px;
  cursor: pointer;
  border-bottom: 1px solid #f4f4f5;
  min-width: 0;
  max-width: 320px;
}

.bg-job-row:last-child {
  border-bottom: none;
}

.bg-job-row:hover {
  background: #f4f4f5;
}

.bg-job-status {
  font-size: 15px;
  color: #a1a1aa;
  flex-shrink: 0;
}

.bg-job-status.running {
  color: #3f3f46;
  animation: bg-job-spin 1s linear infinite;
}

.bg-job-status.exited {
  color: #16a34a;
}

@keyframes bg-job-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.bg-job-command {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-family: var(--cerne-mono);
  color: #18181b;
}

.bg-job-state {
  flex-shrink: 0;
  color: #71717a;
}

.bg-job-stop {
  flex-shrink: 0;
  border: none;
  background: transparent;
  color: #a1a1aa;
  cursor: pointer;
  display: flex;
  align-items: center;
  padding: 2px;
  border-radius: 4px;
}

.bg-job-stop:hover {
  background: #fee2e2;
  color: #b91c1c;
}

.bg-job-output {
  margin: 0;
  max-height: 400px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  font-size: 12px;
  font-family: var(--cerne-mono);
  background: #fafafa;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 10px 12px;
}

.btn-secondary {
  border: var(--cerne-border);
  background: #ffffff;
  color: #52525b;
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}
</style>
