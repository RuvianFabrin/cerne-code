<script setup lang="ts">
import { computed, onUnmounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import Dialog from "primevue/dialog";
import { api, onAnythingProgress, type AnythingDoctorReport } from "../api";

// Verificação e instalação do que o modo Anything precisa: motor (download com hash conferido),
// Ghidra e Java (instalados pelo próprio motor). Só baixa quando o usuário clica.
const visible = defineModel<boolean>("visible", { default: false });
const emit = defineEmits<{ ready: [] }>();
const { t } = useI18n();

const report = ref<AnythingDoctorReport | null>(null);
const busy = ref("");
const error = ref("");
const stage = ref("");
const received = ref(0);
const total = ref<number | null>(null);
let unlisten: (() => void) | null = null;

const percent = computed(() => (total.value && total.value > 0 ? Math.min(100, Math.round((received.value / total.value) * 100)) : null));
const mb = (n: number) => (n / 1048576).toFixed(1);

async function refresh() {
  error.value = "";
  try {
    report.value = await api.anythingDoctor();
    if (report.value.pronto) emit("ready");
  } catch (e) {
    error.value = String(e);
  }
}

async function run(kind: "cli" | "ghidra") {
  busy.value = kind;
  error.value = "";
  stage.value = "";
  received.value = 0;
  total.value = null;
  try {
    if (kind === "cli") await api.anythingInstallCli();
    else await api.anythingInstallGhidra();
  } catch (e) {
    error.value = String(e);
  } finally {
    busy.value = "";
    stage.value = "";
    await refresh();
  }
}

watch(
  visible,
  async (on) => {
    if (on) {
      unlisten = await onAnythingProgress((p) => {
        stage.value = p.stage;
        received.value = p.received;
        total.value = p.total;
      });
      await refresh();
    } else {
      unlisten?.();
      unlisten = null;
    }
  },
  { immediate: true },
);
onUnmounted(() => unlisten?.());

function label(id: string) {
  const key = "anythingSetup.items." + id;
  const text = t(key);
  return text === key ? id : text;
}
function icon(status: string) {
  return status === "ok" ? "check_circle" : status === "aviso" ? "warning" : "error";
}
</script>

<template>
  <Dialog v-model:visible="visible" modal :header="$t('anythingSetup.title')" :style="{ width: 'min(640px, 94vw)' }">
    <p class="as-hint">{{ $t("anythingSetup.hint") }}</p>
    <p v-if="!report && !error" class="as-hint">{{ $t("anythingSetup.checking") }}</p>
    <ul v-if="report" class="as-list">
      <li v-for="i in report.itens" :key="i.id" :class="'as-' + i.status">
        <span class="msi as-icon">{{ icon(i.status) }}</span>
        <div class="as-text">
          <strong>{{ label(i.id) }}</strong>
          <small>{{ i.detalhe }}</small>
        </div>
        <button
          v-if="i.status !== 'ok' && i.acao === 'instalar_cli'"
          class="as-btn"
          :disabled="!!busy"
          @click="run('cli')"
        >{{ $t("anythingSetup.installEngine") }}</button>
        <button
          v-if="i.status !== 'ok' && i.acao === 'instalar_ghidra'"
          class="as-btn"
          :disabled="!!busy"
          @click="run('ghidra')"
        >{{ $t("anythingSetup.installGhidra") }}</button>
      </li>
    </ul>

    <div v-if="busy" class="as-progress">
      <div class="as-stage">{{ stage || $t("anythingSetup.working") }}</div>
      <div class="as-bar"><div class="as-fill" :class="{ indeterminate: percent === null }" :style="percent !== null ? { width: percent + '%' } : {}" /></div>
      <small v-if="percent !== null">{{ mb(received) }} / {{ mb(total || 0) }} MB ({{ percent }}%)</small>
      <small v-else>{{ $t("anythingSetup.noPercent") }}</small>
    </div>

    <p v-if="error" class="as-error">{{ error }}</p>
    <p v-if="report?.pronto" class="as-ok">{{ $t("anythingSetup.allReady") }}</p>

    <template #footer>
      <button class="as-btn" :disabled="!!busy" @click="refresh">{{ $t("anythingSetup.recheck") }}</button>
      <button class="as-btn" @click="visible = false">{{ $t("anythingSetup.close") }}</button>
    </template>
  </Dialog>
</template>

<style scoped>
.as-hint { margin: 0 0 10px; opacity: 0.8; font-size: 13px; }
.as-list { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
.as-list li { display: flex; align-items: center; gap: 10px; padding: 8px 10px; border: 1px solid var(--border, #d0d7de); border-radius: 8px; }
.as-text { flex: 1; display: flex; flex-direction: column; min-width: 0; }
.as-text small { opacity: 0.75; word-break: break-word; }
.as-icon { font-size: 20px; }
.as-ok .as-icon { color: #2e7d32; }
.as-aviso .as-icon { color: #ef8f00; }
.as-falta .as-icon { color: #c62828; }
.as-btn { padding: 4px 12px; border: 1px solid var(--border, #d0d7de); border-radius: 6px; background: transparent; color: inherit; cursor: pointer; }
.as-btn:disabled { opacity: 0.5; cursor: default; }
.as-progress { margin-top: 12px; display: flex; flex-direction: column; gap: 4px; }
.as-bar { height: 8px; border-radius: 4px; background: rgba(128, 128, 128, 0.25); overflow: hidden; }
.as-fill { height: 100%; background: #2e7d32; transition: width 0.2s; }
.as-fill.indeterminate { width: 35%; animation: as-slide 1.2s ease-in-out infinite; }
@keyframes as-slide { 0% { margin-left: -35%; } 100% { margin-left: 100%; } }
.as-error { margin: 10px 0 0; color: #c62828; white-space: pre-wrap; word-break: break-word; }
.as-ok { margin: 10px 0 0; color: #2e7d32; }
</style>
