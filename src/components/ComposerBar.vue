<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { open } from "@tauri-apps/plugin-dialog";
import Select from "primevue/select";
import { api } from "../api";
import { useSessionStore } from "../stores/session";
import ProviderPicker from "./ProviderPicker.vue";
import ContextGauge from "./ContextGauge.vue";
import StatusDot from "./StatusDot.vue";
import ExtraReadPaths from "./ExtraReadPaths.vue";
import { useLlamaHealth } from "../composables/useLlamaHealth";
import type { ExecutionMode, ProviderKind } from "../api";

const EXECUTION_MODE_OPTIONS: { value: ExecutionMode; label: string }[] = [
  { value: "yolo", label: "⚡ YOLO" },
  { value: "auto", label: "Automático" },
  { value: "manual", label: "Manual" },
];

const REASONING_EFFORT_OPTIONS: {
  value: "off" | "auto" | "low" | "medium" | "high";
  label: string;
}[] = [
  // Desligado é o default pra modelos locais: "Auto" deixaria o Qwen3/GLM
  // pensar por conta própria e ficar lento à toa.
  { value: "off", label: "💤 Desligado" },
  { value: "auto", label: "🧠 Auto" },
  { value: "low", label: "🧠 Baixo" },
  { value: "medium", label: "🧠 Médio" },
  { value: "high", label: "🧠 Alto" },
];

function onExecutionModeChange(mode: ExecutionMode) {
  sessionStore.updateExecutionMode(mode);
}

const reasoningEffort = computed(
  () => sessionStore.currentSession?.reasoning_effort ?? "auto",
);

function onReasoningEffortChange(value: "off" | "auto" | "low" | "medium" | "high") {
  sessionStore.updateReasoningEffort(value === "auto" ? null : value);
}

interface Attachment {
  id: string;
  path: string;
  name: string;
  kind: "document" | "image";
  status: "loading" | "ready" | "error";
  text?: string;
  dataUrl?: string;
  error?: string;
  savedMdPath?: string;
  saved?: boolean;
}

const sessionStore = useSessionStore();
const text = ref("");
const textareaRef = ref<HTMLTextAreaElement | null>(null);
const attachments = ref<Attachment[]>([]);
const visionSupported = ref(false);
const savingAttachments = ref(false);

function fileName(path: string) {
  return path.split(/[/\\]/).filter(Boolean).pop() ?? path;
}

// Formatos de imagem com suporte amplo o suficiente pros 4 providers (ver
// README "Pesquisa: suporte real a imagem/áudio/vídeo por provider") — áudio
// e vídeo ficam fora por enquanto.
const IMAGE_EXTENSIONS = ["jpg", "jpeg", "png", "webp"];

function isImagePath(path: string) {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  return IMAGE_EXTENSIONS.includes(ext);
}

// Extensões cobertas por `attachments::extract_text` no backend (pdf/docx/
// xlsx/md/código/txt) + imagem, agora que a checagem de vision por provider
// existe (ver `checkVisionSupport`) — áudio/vídeo continuam fora.
const ATTACHMENT_FILTERS = [
  {
    name: "Documentos e código",
    extensions: [
      "pdf",
      "docx",
      "xlsx",
      "xlsm",
      "xls",
      "ods",
      "md",
      "txt",
      "csv",
      "json",
      "yaml",
      "yml",
      "toml",
      "rs",
      "ts",
      "tsx",
      "js",
      "jsx",
      "vue",
      "py",
      "go",
      "java",
      "c",
      "cpp",
      "h",
      "hpp",
      "cs",
      "rb",
      "php",
      "swift",
      "kt",
      "sh",
      "sql",
      "html",
      "css",
    ],
  },
  { name: "Imagens", extensions: IMAGE_EXTENSIONS },
];

async function refreshVisionSupport() {
  if (!sessionStore.currentId) {
    visionSupported.value = false;
    return;
  }
  try {
    visionSupported.value = await api.checkVisionSupport(sessionStore.currentId);
  } catch {
    visionSupported.value = false;
  }
}

watch(() => sessionStore.currentSession, refreshVisionSupport, { immediate: true });

// Sempre resolve o item de volta pelo array reativo antes de mutar — mutar a
// referência do objeto que foi guardada ANTES do `push` mexe no objeto cru,
// não no proxy reativo que o Vue de fato observa, então a UI nunca atualiza
// (o chip ficava preso em "loading" pra sempre - bug real encontrado testando
// ao vivo).
function updateAttachment(id: string, patch: Partial<Attachment>) {
  const a = attachments.value.find((x) => x.id === id);
  if (a) Object.assign(a, patch);
}

function extractImage(id: string, path: string) {
  if (!visionSupported.value) {
    updateAttachment(id, {
      status: "error",
      error: "O provider/modelo desta sessão não tem suporte a visão configurado — a imagem não vai ser enviada.",
    });
    return;
  }
  api
    .readImageAsDataUrl(path)
    .then((dataUrl) => updateAttachment(id, { status: "ready", dataUrl }))
    .catch((e) => updateAttachment(id, { status: "error", error: String(e) }));
}

function extractDocument(id: string, path: string) {
  api
    .extractAttachmentText(path)
    .then((extracted) => updateAttachment(id, { status: "ready", text: extracted }))
    .catch((e) => updateAttachment(id, { status: "error", error: String(e) }));
}

async function addAttachments() {
  const selected = await open({ directory: false, multiple: true, filters: ATTACHMENT_FILTERS });
  if (!selected) return;
  const paths = Array.isArray(selected) ? selected : [selected];
  for (const path of paths) {
    const id = crypto.randomUUID();
    if (isImagePath(path)) {
      attachments.value.push({ id, path, name: fileName(path), kind: "image", status: "loading" });
      extractImage(id, path);
    } else {
      attachments.value.push({ id, path, name: fileName(path), kind: "document", status: "loading" });
      extractDocument(id, path);
    }
  }
}

/** Cola imagem direto da área de transferência (Ctrl+V no textarea) — não
 * passa por caminho de arquivo real, então o data URL é montado no próprio
 * navegador via `FileReader`, sem precisar do comando Tauri de leitura de
 * disco. É o fluxo que a maioria das pessoas realmente usa pra anexar
 * screenshot, então tem que funcionar sem passar pelo seletor de arquivo. */
function onPaste(e: ClipboardEvent) {
  const items = e.clipboardData?.items;
  if (!items) return;
  for (const item of items) {
    if (!item.type.startsWith("image/")) continue;
    const file = item.getAsFile();
    if (!file) continue;
    e.preventDefault();
    const id = crypto.randomUUID();
    const ext = file.type.split("/")[1] || "png";
    attachments.value.push({ id, path: "", name: `colado-${Date.now()}.${ext}`, kind: "image", status: "loading" });
    if (!visionSupported.value) {
      updateAttachment(id, {
        status: "error",
        error: "O provider/modelo desta sessão não tem suporte a visão configurado — a imagem não vai ser enviada.",
      });
      continue;
    }
    const reader = new FileReader();
    reader.onload = () => updateAttachment(id, { status: "ready", dataUrl: reader.result as string });
    reader.onerror = () => updateAttachment(id, { status: "error", error: "Falha ao ler a imagem colada" });
    reader.readAsDataURL(file);
  }
}

function removeAttachment(id: string) {
  attachments.value = attachments.value.filter((a) => a.id !== id);
}

function buildMessageWithAttachments(userText: string): string {
  const ready = attachments.value.filter((a) => a.kind === "document" && a.status === "ready" && a.text);
  if (ready.length === 0) return userText;
  const blocks = ready.map((a) => {
    const charCount = a.text!.length;
    const lineCount = a.text!.split("\n").length;
    if (a.savedMdPath) {
      return [
        `### Anexo: ${a.name}`,
        `O arquivo do usuario foi convertido para .md e salvo em: ${a.savedMdPath}`,
        `Tamanho: ${charCount.toLocaleString("pt-BR")} caracteres, ${lineCount.toLocaleString("pt-BR")} linhas.`,
        ``,
        `INSTRUCOES DE LEITURA (obrigatorio seguir):`,
        `- NAO tente ler o arquivo inteiro de uma vez — ele e grande e vai desperdicar tokens.`,
        `- Use read_file(path="${a.savedMdPath}", offset=0, limit=200) para ler as primeiras 200 linhas.`,
        `- Use offset+limit para navegar pelo conteudo aos poucos (ex: offset=200, limit=200 para as proximas 200 linhas).`,
        `- Use grep(pattern="...", path="${a.savedMdPath}") para buscar termos especificos sem ler tudo.`,
        `- Combine grep + read_file com offset/limit para encontrar e ler so as partes relevantes.`,
      ].join("\n");
    }
    return `### Anexo: ${a.name}\n\n${a.text}`;
  });
  return `${blocks.join("\n\n")}\n\n${userText}`;
}

function collectImages(): string[] {
  return attachments.value.filter((a) => a.kind === "image" && a.status === "ready" && a.dataUrl).map((a) => a.dataUrl!);
}

// Local, editable picker state — deliberately NOT a computed straight off
// currentSession. A session is only ever persisted with provider+model
// together (never a provider with no model), so while the user is mid-pick
// the on-screen selection has to be allowed to disagree with what's saved.
const pendingProvider = ref<ProviderKind>(sessionStore.currentSession?.provider ?? "ollama");
const pendingFork = ref(sessionStore.currentFork);
const pendingCustomProviderId = ref(sessionStore.currentCustomProviderId);
const pendingModel = ref<string | null>(sessionStore.currentSession?.model || null);

// Watching currentSession (not currentId) on purpose — currentId flips
// synchronously on selectSession, but the session data itself only lands
// once reloadCurrent's fetch resolves; syncing off currentId would show a
// stale provider/model for a beat.
watch(
  () => sessionStore.currentSession,
  (session) => {
    pendingProvider.value = session?.provider ?? "ollama";
    pendingFork.value = session?.llama_fork ?? sessionStore.currentFork;
    pendingCustomProviderId.value = session?.custom_provider_id ?? sessionStore.currentCustomProviderId;
    pendingModel.value = session?.model || null;
  },
);

function onProviderChange(kind: ProviderKind) {
  pendingProvider.value = kind;
  pendingModel.value = null; // model list differs per provider, don't carry the old id over
}

function onForkChange(forkId: string) {
  pendingFork.value = forkId;
  sessionStore.currentFork = forkId;
  pendingModel.value = null;
}

function onCustomProviderIdChange(id: string) {
  pendingCustomProviderId.value = id;
  sessionStore.currentCustomProviderId = id;
  pendingModel.value = null;
}

function onModelChange(id: string) {
  pendingModel.value = id;
  sessionStore.updateProviderModel(pendingProvider.value, id, pendingFork.value, pendingCustomProviderId.value);
}

const healthTargetFork = computed(() => (pendingProvider.value === "llama_cpp" ? pendingFork.value : null));
const { isUp: llamaIsUp } = useLlamaHealth(healthTargetFork);

function grow() {
  const el = textareaRef.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = Math.min(el.scrollHeight, 240) + "px";
}

function buildDisplayText(userText: string): string {
  const ready = attachments.value.filter((a) => a.status === "ready");
  if (ready.length === 0) return userText;
  const names = ready.map((a) => `${a.kind === "image" ? "🖼️" : "📎"} ${a.name}`).join("  ");
  return `${userText}\n\n${names}`;
}

async function submit() {
  const value = text.value;
  if (!value.trim() || sessionStore.status !== "idle") return;
  if (attachments.value.some((a) => a.status === "loading")) return;
  const sessionId = sessionStore.currentId;
  const docsToSave = attachments.value.filter((a) => a.kind === "document" && a.status === "ready" && a.text && !a.savedMdPath);
  if (sessionId && docsToSave.length > 0) {
    savingAttachments.value = true;
    for (const a of docsToSave) {
      try {
        a.savedMdPath = await api.saveAttachmentMd(sessionId, a.name, a.text!);
        a.saved = true;
      } catch {
        // fallback: embed full text if save fails
      }
    }
    savingAttachments.value = false;
    await new Promise((r) => setTimeout(r, 1200));
  }
  const message = buildMessageWithAttachments(value);
  const displayText = buildDisplayText(value);
  const images = collectImages();
  text.value = "";
  attachments.value = [];
  await sessionStore.send(message, displayText, images);
  grow();
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    submit();
  }
}

watch(
  () => sessionStore.draftText,
  (draft) => {
    if (!draft) return;
    text.value = draft;
    sessionStore.draftText = "";
    nextTick(() => {
      grow();
      textareaRef.value?.focus();
    });
  },
);
</script>

<template>
  <div class="composer">
    <div class="composer-toolbar">
      <StatusDot v-if="pendingProvider === 'llama_cpp'" :up="llamaIsUp" />
      <ProviderPicker
        collapsible
        :provider="pendingProvider"
        :fork="pendingFork"
        :custom-provider-id="pendingCustomProviderId"
        :model="pendingModel"
        @update:provider="onProviderChange"
        @update:fork="onForkChange"
        @update:custom-provider-id="onCustomProviderIdChange"
        @update:model="onModelChange"
      />
      <ExtraReadPaths v-if="sessionStore.currentSession?.project_root" />
    </div>
    <div v-if="attachments.length" class="attachments-row">
      <div
        v-for="a in attachments"
        :key="a.id"
        class="attachment-chip"
        :class="{ error: a.status === 'error', saving: savingAttachments && a.kind === 'document' && a.status === 'ready' && !a.savedMdPath, saved: a.saved }"
        v-tooltip.top="a.status === 'error' ? `${a.path}\n${a.error}` : a.path"
      >
        <span class="msi spin" v-if="a.status === 'loading' || (savingAttachments && a.kind === 'document' && !a.savedMdPath)">progress_activity</span>
        <span class="msi" v-else-if="a.saved">check_circle</span>
        <span class="msi" v-else-if="a.status === 'error'">error</span>
        <img v-else-if="a.kind === 'image' && a.dataUrl" :src="a.dataUrl" class="attachment-thumb" alt="" />
        <span class="msi" v-else>description</span>
        <span class="attachment-name">{{ savingAttachments && a.kind === 'document' && !a.savedMdPath ? 'Otimizando...' : a.saved ? 'Pronto!' : a.name }}</span>
        <button class="attachment-remove" v-tooltip.top="'Remover'" @click="removeAttachment(a.id)" :disabled="savingAttachments">
          <span class="msi">close</span>
        </button>
      </div>
    </div>
    <div v-if="savingAttachments" class="saving-hint">
      <span class="msi spin">progress_activity</span>
      Aguarde, otimizando arquivo para economizar tokens...
    </div>
    <textarea
      ref="textareaRef"
      v-model="text"
      class="composer-input"
      rows="1"
      placeholder="Peça pro Cerne Code ler, editar ou rodar algo no seu projeto..."
      @input="grow"
      @keydown="onKeydown"
      @paste="onPaste"
    />
    <div class="composer-footer">
      <div class="footer-left">
        <button class="attach-btn" v-tooltip.top="'Anexar arquivo'" @click="addAttachments">
          <span class="msi">add</span>
        </button>
        <Select
          v-if="sessionStore.currentSession"
          :modelValue="sessionStore.currentSession.execution_mode"
          @update:modelValue="(v) => onExecutionModeChange(v as ExecutionMode)"
          :options="EXECUTION_MODE_OPTIONS"
          optionLabel="label"
          optionValue="value"
          class="execution-mode-select"
          size="small"
          v-tooltip.top="'YOLO: edita direto, sem sandbox. Automático: sandbox + você aceita/rejeita. Manual: aprova cada ação.'"
        />
        <Select
          v-if="sessionStore.currentSession"
          :modelValue="reasoningEffort"
          @update:modelValue="(v) => onReasoningEffortChange(v as 'off' | 'auto' | 'low' | 'medium' | 'high')"
          :options="REASONING_EFFORT_OPTIONS"
          optionLabel="label"
          optionValue="value"
          class="execution-mode-select"
          size="small"
          v-tooltip.top="'Raciocínio do modelo. 💤 Desligado = sem thinking (mais rápido, default nos locais). Auto = o modelo decide.'"
        />
        <button
          v-if="sessionStore.currentSession"
          class="fable-btn"
          :class="{ 'fable-on': sessionStore.currentSession.fable_method }"
          v-tooltip.top="'Método Fable (p/ modelos pequenos/médios): liga um loop classificar → agir → verificar pra o modelo não abandonar tarefas. Desligado por padrão; em modelos grandes só infla o prompt.'"
          @click="
            sessionStore.updateFableMethod(!sessionStore.currentSession.fable_method)
          "
        >
          <span class="msi">route</span>
        </button>
      </div>
      <div class="footer-right">
        <ContextGauge />
        <button
          v-if="sessionStore.status === 'idle'"
          class="send-btn"
          :disabled="!text.trim() || savingAttachments"
          @click="submit"
        >
          <span class="msi">arrow_upward</span>
        </button>
        <button v-else class="send-btn stop-btn" v-tooltip.top="'Cancelar execução'" @click="sessionStore.cancelTurn()">
          <span class="msi">stop</span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.composer {
  border: var(--cerne-border);
  border-radius: 14px;
  padding: 10px 12px 8px;
  background: #ffffff;
  max-width: 100%;
  box-sizing: border-box;
}

.composer-toolbar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding-bottom: 8px;
  margin-bottom: 8px;
  border-bottom: var(--cerne-border);
}

.composer-input {
  width: 100%;
  border: none;
  outline: none;
  resize: none;
  font-size: 14px;
  font-weight: 500;
  font-family: inherit;
  color: #18181b;
  line-height: 1.5;
  max-height: 240px;
}

.composer-input::placeholder {
  color: #a1a1aa;
}

.attachments-row {
  display: flex;
  flex-wrap: wrap;
  gap: 6px;
  padding-bottom: 8px;
}

.attachment-chip {
  display: flex;
  align-items: center;
  gap: 5px;
  max-width: 200px;
  background: #f4f4f5;
  border-radius: 999px;
  padding: 4px 6px 4px 9px;
  font-size: 12px;
  font-weight: 500;
  color: #3f3f46;
}

.attachment-chip.error {
  background: #fee2e2;
  color: #b91c1c;
}

.attachment-chip.saving {
  background: #eff6ff;
  color: #1d4ed8;
}

.attachment-chip.saved {
  background: #ecfdf3;
  color: #15803d;
}

.attachment-chip.saved .msi {
  color: #22c55e;
}

.saving-hint {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 400;
  color: #3b82f6;
  padding: 4px 0 8px;
}

.saving-hint .msi {
  font-size: 14px;
}

.attachment-chip .msi {
  font-size: 14px;
  flex-shrink: 0;
}

.attachment-thumb {
  width: 16px;
  height: 16px;
  border-radius: 3px;
  object-fit: cover;
  flex-shrink: 0;
}

.attachment-chip .spin {
  animation: attach-spin 1s linear infinite;
}

@keyframes attach-spin {
  from {
    transform: rotate(0deg);
  }
  to {
    transform: rotate(360deg);
  }
}

.attachment-name {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  min-width: 0;
}

.attachment-remove {
  border: none;
  background: transparent;
  color: #a1a1aa;
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  opacity: 0.6;
  border-radius: 4px;
  padding: 1px;
}

.attachment-remove:hover {
  opacity: 1;
  color: #ef4444;
}

.attachment-remove .msi {
  font-size: 13px;
}

.composer-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-top: 6px;
  flex-wrap: wrap;
  row-gap: 6px;
}

.footer-left,
.footer-right {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
  min-width: 0;
}

.footer-right {
  justify-content: flex-end;
  margin-left: auto;
}

.execution-mode-select {
  font-size: 12px;
  font-weight: 500;
}

.execution-mode-select :deep(.p-select) {
  border-radius: 8px;
}

.execution-mode-select :deep(.p-select-label) {
  padding: 5px 8px;
  font-size: 12px;
  font-weight: 500;
}

.attach-btn {
  border: var(--cerne-border);
  background: #ffffff;
  border-radius: 8px;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: #52525b;
}

.attach-btn:disabled {
  opacity: 0.5;
  cursor: default;
}

.fable-btn {
  border: var(--cerne-border);
  background: #ffffff;
  border-radius: 8px;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: #52525b;
}

.fable-btn:hover {
  border-color: #d4d4d8;
}

/* Ligado = chip rosa, pra deixar óbvio que o método Fable está ativo
   (é opt-in, só faz sentido em modelos pequenos/médios). */
.fable-btn.fable-on {
  background: #db2777;
  border-color: #db2777;
  color: #ffffff;
}

.send-btn {
  border: none;
  background: #18181b;
  color: #ffffff;
  border-radius: 8px;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
}

.send-btn:disabled {
  background: #e4e4e7;
  color: #a1a1aa;
  cursor: default;
}

.stop-btn {
  background: #dc2626;
}

.stop-btn:hover {
  background: #b91c1c;
}

.send-btn .msi,
.attach-btn .msi,
.fable-btn .msi {
  font-size: 18px;
}
</style>
