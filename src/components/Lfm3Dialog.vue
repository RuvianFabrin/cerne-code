<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import Dialog from "primevue/dialog";
import { useI18n } from "vue-i18n";
import { api } from "../api";
import type { Lfm3Message, Lfm3ModelStatus, Lfm3ClassifyResult } from "../api";
import { useProviderStore } from "../stores/provider";
import { fileToDataUri, startRecording } from "../lib/wav";
import type { Recording } from "../lib/wav";

const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{
  "update:visible": [value: boolean];
  select: [payload: { fork: string; model: string }];
}>();

const { t } = useI18n();
const providerStore = useProviderStore();

/** Id fixo do fork LFM3 dentro do `providers.toml` do Cerne. */
const FORK_ID = "lfm3";
const DECISION = "d1-omni-2b";

const LANGS = ["auto", "pt", "en", "es", "fr", "de", "it", "zh", "ja", "ko", "ru", "ar", "nl"];

// --- Configuração do fork -------------------------------------------------

const forkSetup = ref({
  label: "LFM3 (LiquidAI)",
  serverExe: "",
  modelsIni: "",
  port: 8090,
});
const setupError = ref("");
const setupSaved = ref(false);

const forkExists = computed(() => providerStore.forks.some((f) => f.id === FORK_ID));

async function saveFork() {
  setupError.value = "";
  setupSaved.value = false;
  if (!forkSetup.value.serverExe.trim() || !forkSetup.value.modelsIni.trim()) {
    setupError.value = t("lfm3.errors.fillPaths");
    return;
  }
  try {
    await providerStore.addLlamaFork({
      id: FORK_ID,
      label: forkSetup.value.label.trim() || "LFM3",
      server_exe: forkSetup.value.serverExe.trim(),
      models_ini: forkSetup.value.modelsIni.trim(),
      port: Number(forkSetup.value.port) || 8090,
    });
    setupSaved.value = true;
    await loadCatalog();
  } catch (e) {
    setupError.value = String(e);
  }
}

// --- Catálogo (estado dos 5 modelos) -------------------------------------

const catalog = ref<Lfm3ModelStatus[]>([]);
const catalogError = ref("");

async function loadCatalog() {
  catalogError.value = "";
  if (!forkExists.value) {
    catalog.value = [];
    return;
  }
  try {
    catalog.value = await api.lfm3Catalog(FORK_ID);
  } catch (e) {
    catalog.value = [];
    catalogError.value = String(e);
  }
}

function statusOf(id: string): Lfm3ModelStatus | undefined {
  return catalog.value.find((m) => m.id === id);
}

const chatModels = computed(() => catalog.value.filter((m) => m.chat));

// --- Cartões dos modelos --------------------------------------------------

const MODEL_CARDS = [
  { id: "lfm3-1.7b-text", title: "LFM3-1.7B", kind: "text" },
  { id: "lfm3-vl-2b", title: "LFM3-VL-2B", kind: "vision" },
  { id: "lfm3-audio-2b", title: "LFM3-Audio-2B", kind: "audio" },
  { id: "lfm3-omni-2b", title: "LFM3-Omni-2B", kind: "omni" },
  { id: DECISION, title: "d1-Omni-2B", kind: "decision" },
] as const;

function useInChat(modelId: string) {
  emit("select", { fork: FORK_ID, model: modelId });
  emit("update:visible", false);
}

// --- Testes de chat (texto / imagem) --------------------------------------

const chatModel = ref("lfm3-1.7b-text");
const chatPrompt = ref("");
const chatImage = ref<string | null>(null);
const chatImageName = ref("");
const chatReply = ref("");
const chatError = ref("");
const chatBusy = ref(false);

watch(chatModels, (list) => {
  if (!list.some((m) => m.id === chatModel.value) && list.length) chatModel.value = list[0].id;
});

async function pickImage(event: Event) {
  const file = (event.target as HTMLInputElement).files?.[0];
  if (!file) return;
  chatImage.value = await fileToDataUri(file);
  chatImageName.value = file.name;
}

async function runChatTest() {
  chatError.value = "";
  chatReply.value = "";
  const prompt = chatPrompt.value.trim() || t("lfm3.test.defaultPrompt");
  const wantsImage = chatImage.value !== null;
  if (wantsImage && !statusOf(chatModel.value)?.image) {
    chatError.value = t("lfm3.errors.noImageModel");
    return;
  }
  const content: Lfm3Message["content"] = wantsImage
    ? [
        { type: "image_url", image_url: { url: chatImage.value } },
        { type: "text", text: prompt },
      ]
    : prompt;
  chatBusy.value = true;
  try {
    chatReply.value = await api.lfm3Chat(FORK_ID, chatModel.value, [{ role: "user", content }], 400);
  } catch (e) {
    chatError.value = String(e);
  } finally {
    chatBusy.value = false;
  }
}

// --- Voz: transcrever (ASR) ou conversar (áudio direto no Omni/Audio) -----

const lang = ref("pt");
const voice = ref("female");
const voiceMode = ref<"transcribe" | "reply">("transcribe");
const voiceModel = ref("lfm3-omni-2b");
const recording = ref<Recording | null>(null);
const voiceText = ref("");
const voiceError = ref("");
const voiceBusy = ref(false);
const spokenUrl = ref("");

async function toggleRecording() {
  voiceError.value = "";
  if (recording.value) {
    const current = recording.value;
    recording.value = null;
    voiceBusy.value = true;
    try {
      const dataUri = await current.stop();
      voiceText.value = await runVoice(dataUri);
    } catch (e) {
      voiceError.value = String(e);
    } finally {
      voiceBusy.value = false;
    }
    return;
  }
  try {
    recording.value = await startRecording();
  } catch (e) {
    voiceError.value = t("lfm3.errors.micDenied", { error: String(e) });
  }
}

async function runVoice(dataUri: string): Promise<string> {
  if (voiceMode.value === "transcribe") {
    return api.lfm3Transcribe(FORK_ID, dataUri, lang.value);
  }
  const base64 = dataUri.split("base64,")[1] ?? "";
  const messages: Lfm3Message[] = [
    { role: "system", content: t("lfm3.voice.replySystem") },
    { role: "user", content: [{ type: "input_audio", input_audio: { data: base64, format: "wav" } }] },
  ];
  return api.lfm3Chat(FORK_ID, voiceModel.value, messages, 400);
}

async function speakText() {
  if (!voiceText.value.trim()) return;
  voiceError.value = "";
  voiceBusy.value = true;
  try {
    const b64 = await api.lfm3Speak(FORK_ID, voiceText.value, lang.value, voice.value);
    spokenUrl.value = `data:audio/wav;base64,${b64}`;
  } catch (e) {
    voiceError.value = String(e);
  } finally {
    voiceBusy.value = false;
  }
}

// --- Classificador d1-Omni (não é chat) -----------------------------------

const DEFAULT_CONTEXT = "I was charged twice this month, please refund one of them.";
const DEFAULT_QUESTIONS = JSON.stringify(
  {
    refund: { type: "noul", instructions: "Is the customer asking for a refund?" },
    team: {
      type: "choice",
      instructions: "Which team should handle this?",
      criteria: {
        billing: "Charges, refunds, invoices",
        technical: "App or site faults",
        fraud: "Suspected unauthorised use",
      },
    },
  },
  null,
  2,
);
const classifyContext = ref(DEFAULT_CONTEXT);
const classifyQuestions = ref(DEFAULT_QUESTIONS);
const classifyResult = ref<Lfm3ClassifyResult | null>(null);
const classifyError = ref("");
const classifyBusy = ref(false);

async function runClassify() {
  classifyError.value = "";
  classifyResult.value = null;
  let questions: Record<string, unknown>;
  try {
    questions = JSON.parse(classifyQuestions.value);
  } catch {
    classifyError.value = t("lfm3.errors.badJson");
    return;
  }
  classifyBusy.value = true;
  try {
    classifyResult.value = await api.lfm3Classify(FORK_ID, classifyContext.value, questions, [], []);
  } catch (e) {
    classifyError.value = String(e);
  } finally {
    classifyBusy.value = false;
  }
}

function choiceRanking(probabilities: Record<string, number> | undefined): string {
  if (!probabilities) return "";
  return Object.entries(probabilities)
    .sort((a, b) => b[1] - a[1])
    .map(([k, v]) => `${k} ${(v * 100).toFixed(1)}%`)
    .join(" · ");
}

onMounted(() => {
  if (props.visible) void loadCatalog();
});
watch(
  () => props.visible,
  (open) => {
    if (open) void loadCatalog();
  },
);
watch(forkExists, (exists) => {
  if (exists) void loadCatalog();
});
</script>

<template>
  <Dialog
    :visible="props.visible"
    @update:visible="(v) => emit('update:visible', v)"
    :header="t('lfm3.title')"
    modal
    :style="{ width: 'min(920px, 96vw)' }"
    :dismissableMask="true"
  >
    <div class="lfm3">
      <p class="lfm3-intro">{{ t("lfm3.intro") }}</p>

      <!-- 1. Como usar -->
      <section class="lfm3-section">
        <h3>{{ t("lfm3.howTo.title") }}</h3>
        <ol class="lfm3-steps">
          <li>{{ t("lfm3.howTo.step1") }}</li>
          <li>{{ t("lfm3.howTo.step2") }}</li>
          <li>{{ t("lfm3.howTo.step3") }}</li>
          <li>{{ t("lfm3.howTo.step4") }}</li>
        </ol>
      </section>

      <!-- 2. Configuração do fork -->
      <section class="lfm3-section">
        <h3>{{ t("lfm3.setup.title") }}</h3>
        <p class="lfm3-hint">{{ t("lfm3.setup.hint") }}</p>
        <div class="lfm3-form">
          <label>
            {{ t("lfm3.setup.label") }}
            <input v-model="forkSetup.label" type="text" />
          </label>
          <label>
            {{ t("lfm3.setup.serverExe") }}
            <input v-model="forkSetup.serverExe" type="text" placeholder="C:\...\llama-server.exe" />
          </label>
          <label>
            {{ t("lfm3.setup.modelsIni") }}
            <input v-model="forkSetup.modelsIni" type="text" placeholder="C:\...\models-lfm3.ini" />
          </label>
          <label>
            {{ t("lfm3.setup.port") }}
            <input v-model.number="forkSetup.port" type="number" min="1" max="65535" />
          </label>
        </div>
        <div class="lfm3-row">
          <button type="button" class="btn-primary" @click="saveFork">
            {{ forkExists ? t("lfm3.setup.update") : t("lfm3.setup.create") }}
          </button>
          <span v-if="setupSaved" class="lfm3-ok">{{ t("lfm3.setup.saved") }}</span>
          <span v-if="setupError" class="lfm3-error">{{ setupError }}</span>
        </div>
        <p v-if="catalogError" class="lfm3-error">{{ catalogError }}</p>
      </section>

      <!-- 3. Os 5 modelos -->
      <section class="lfm3-section">
        <h3>{{ t("lfm3.models.title") }}</h3>
        <div class="lfm3-cards">
          <article v-for="card in MODEL_CARDS" :key="card.id" class="lfm3-card">
            <header>
              <strong>{{ card.title }}</strong>
              <span class="lfm3-badges">
                <span v-if="card.kind === 'decision'" class="lfm3-badge decision">{{ t("lfm3.badges.decision") }}</span>
                <span v-else class="lfm3-badge">{{ t("lfm3.badges.chat") }}</span>
                <span v-if="card.kind === 'vision' || card.kind === 'omni' || card.kind === 'decision'" class="lfm3-badge">{{ t("lfm3.badges.image") }}</span>
                <span v-if="card.kind === 'audio' || card.kind === 'omni' || card.kind === 'decision'" class="lfm3-badge">{{ t("lfm3.badges.audio") }}</span>
                <span v-if="card.kind === 'audio' || card.kind === 'omni'" class="lfm3-badge">{{ t("lfm3.badges.speech") }}</span>
              </span>
            </header>
            <p>{{ t(`lfm3.models.${card.kind}`) }}</p>
            <p class="lfm3-when"><em>{{ t(`lfm3.models.when.${card.kind}`) }}</em></p>
            <p class="lfm3-status">
              <template v-if="!forkExists">{{ t("lfm3.status.noFork") }}</template>
              <template v-else-if="!statusOf(card.id)?.configured">{{ t("lfm3.status.notInIni") }}</template>
              <template v-else-if="!statusOf(card.id)?.files_exist">{{ t("lfm3.status.missingFiles") }}</template>
              <template v-else>{{ t("lfm3.status.ready") }}</template>
            </p>
            <button
              v-if="card.kind !== 'decision'"
              type="button"
              class="btn-secondary"
              :disabled="!forkExists"
              @click="useInChat(card.id)"
            >
              {{ t("lfm3.models.useInChat") }}
            </button>
            <p v-else class="lfm3-hint">{{ t("lfm3.models.decisionNote") }}</p>
          </article>
        </div>
      </section>

      <!-- 4. Testes rápidos -->
      <section class="lfm3-section">
        <h3>{{ t("lfm3.test.title") }}</h3>
        <p class="lfm3-hint">{{ t("lfm3.test.hint") }}</p>

        <div class="lfm3-subsection">
          <h4>{{ t("lfm3.test.chatTitle") }}</h4>
          <div class="lfm3-row">
            <select v-model="chatModel" :disabled="!chatModels.length">
              <option v-for="m in chatModels" :key="m.id" :value="m.id">{{ m.id }}</option>
            </select>
            <input type="file" accept="image/*" @change="pickImage" />
            <span v-if="chatImageName" class="lfm3-hint">{{ chatImageName }}</span>
          </div>
          <textarea v-model="chatPrompt" rows="2" :placeholder="t('lfm3.test.defaultPrompt')"></textarea>
          <div class="lfm3-row">
            <button type="button" class="btn-primary" :disabled="chatBusy || !forkExists" @click="runChatTest">
              {{ chatBusy ? t("lfm3.test.running") : t("lfm3.test.run") }}
            </button>
          </div>
          <pre v-if="chatReply" class="lfm3-output">{{ chatReply }}</pre>
          <p v-if="chatError" class="lfm3-error">{{ chatError }}</p>
        </div>

        <div class="lfm3-subsection">
          <h4>{{ t("lfm3.test.voiceTitle") }}</h4>
          <div class="lfm3-row">
            <label>
              {{ t("lfm3.voice.language") }}
              <select v-model="lang">
                <option v-for="code in LANGS" :key="code" :value="code">{{ t(`lfm3.lang.${code}`) }}</option>
              </select>
            </label>
            <label>
              {{ t("lfm3.voice.voice") }}
              <select v-model="voice">
                <option value="female">{{ t("lfm3.voice.female") }}</option>
                <option value="male">{{ t("lfm3.voice.male") }}</option>
              </select>
            </label>
            <label>
              {{ t("lfm3.voice.mode") }}
              <select v-model="voiceMode">
                <option value="transcribe">{{ t("lfm3.voice.modeTranscribe") }}</option>
                <option value="reply">{{ t("lfm3.voice.modeReply") }}</option>
              </select>
            </label>
            <label v-if="voiceMode === 'reply'">
              {{ t("lfm3.voice.replyModel") }}
              <select v-model="voiceModel">
                <option value="lfm3-omni-2b">lfm3-omni-2b</option>
                <option value="lfm3-audio-2b">lfm3-audio-2b</option>
              </select>
            </label>
          </div>
          <div class="lfm3-row">
            <button type="button" class="btn-primary" :disabled="voiceBusy || !forkExists" @click="toggleRecording">
              {{ recording ? t("lfm3.voice.stop") : t("lfm3.voice.record") }}
            </button>
            <span v-if="voiceBusy" class="lfm3-hint">{{ t("lfm3.test.running") }}</span>
          </div>
          <div v-if="voiceText" class="lfm3-row">
            <pre class="lfm3-output">{{ voiceText }}</pre>
            <button type="button" class="btn-secondary" :disabled="voiceBusy" @click="speakText">
              {{ t("lfm3.voice.speak") }}
            </button>
          </div>
          <audio v-if="spokenUrl" :src="spokenUrl" controls></audio>
          <p v-if="voiceError" class="lfm3-error">{{ voiceError }}</p>
        </div>

        <div class="lfm3-subsection">
          <h4>{{ t("lfm3.test.classifyTitle") }}</h4>
          <p class="lfm3-hint">{{ t("lfm3.test.classifyHint") }}</p>
          <textarea v-model="classifyContext" rows="2"></textarea>
          <textarea v-model="classifyQuestions" rows="8" class="lfm3-mono"></textarea>
          <div class="lfm3-row">
            <button type="button" class="btn-primary" :disabled="classifyBusy || !forkExists" @click="runClassify">
              {{ classifyBusy ? t("lfm3.test.running") : t("lfm3.test.run") }}
            </button>
          </div>
          <div v-if="classifyResult" class="lfm3-output-list">
            <div v-for="(answer, key) in classifyResult.answers" :key="key">
              <strong>{{ key }}</strong>
              <span v-if="answer.type === 'choice'"> → {{ answer.choice }} ({{ choiceRanking(answer.probabilities) }})</span>
              <span v-else> → {{ JSON.stringify(answer) }}</span>
            </div>
          </div>
          <p v-if="classifyError" class="lfm3-error">{{ classifyError }}</p>
        </div>
      </section>

      <!-- 5. Quando usar junto com o chat -->
      <section class="lfm3-section">
        <h3>{{ t("lfm3.useCases.title") }}</h3>
        <p class="lfm3-hint">{{ t("lfm3.useCases.intro") }}</p>
        <ul class="lfm3-steps">
          <li v-for="n in 5" :key="n"><strong>{{ t(`lfm3.useCases.item${n}.title`) }}</strong> — {{ t(`lfm3.useCases.item${n}.body`) }}</li>
        </ul>
      </section>
    </div>
  </Dialog>
</template>

<style scoped>
.lfm3 { display: flex; flex-direction: column; gap: 18px; font-size: 14px; }
.lfm3-intro { margin: 0; }
.lfm3-section h3 { margin: 0 0 8px; font-size: 16px; }
.lfm3-subsection { border-top: 1px solid var(--p-content-border-color, #3333); padding-top: 12px; margin-top: 12px; }
.lfm3-subsection h4 { margin: 0 0 8px; font-size: 14px; }
.lfm3-steps { margin: 0; padding-left: 20px; display: flex; flex-direction: column; gap: 4px; }
.lfm3-hint { opacity: 0.75; font-size: 12px; margin: 0 0 8px; }
.lfm3-form { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 10px; }
.lfm3-form label, .lfm3-row label { display: flex; flex-direction: column; gap: 4px; font-size: 12px; }
.lfm3-form input, .lfm3-row select, .lfm3-row input[type="file"] { font: inherit; }
.lfm3-row { display: flex; flex-wrap: wrap; gap: 10px; align-items: center; margin: 8px 0; }
.lfm3-cards { display: grid; grid-template-columns: repeat(auto-fit, minmax(190px, 1fr)); gap: 10px; }
.lfm3-card { border: 1px solid #8884; border-radius: 10px; padding: 10px; display: flex; flex-direction: column; gap: 6px; }
.lfm3-card header { display: flex; flex-direction: column; gap: 6px; }
.lfm3-card p { margin: 0; font-size: 12px; }
.lfm3-when { opacity: 0.85; }
.lfm3-status { opacity: 0.8; }
.lfm3-badges { display: flex; flex-wrap: wrap; gap: 4px; }
.lfm3-badge { font-size: 11px; border: 1px solid #8886; border-radius: 999px; padding: 1px 8px; }
.lfm3-badge.decision { border-color: #c9a227; }
.lfm3-output { white-space: pre-wrap; background: #8881; border-radius: 8px; padding: 8px; margin: 8px 0 0; font: inherit; }
.lfm3-output-list { display: flex; flex-direction: column; gap: 4px; margin-top: 8px; font-size: 13px; }
.lfm3-mono { font-family: Consolas, monospace; font-size: 12px; width: 100%; }
textarea { width: 100%; font: inherit; }
.lfm3-error { color: #d9534f; margin: 6px 0 0; }
.lfm3-ok { color: #2e8b57; }
</style>
