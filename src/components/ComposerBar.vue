<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import Select from "primevue/select";
import Dialog from "primevue/dialog";
import Popover from "primevue/popover";
import { api } from "../api";
import { useSessionStore } from "../stores/session";
import { useProviderStore } from "../stores/provider";
import ProviderPicker from "./ProviderPicker.vue";
import ContextGauge from "./ContextGauge.vue";
import StatusDot from "./StatusDot.vue";
import ExtraReadPaths from "./ExtraReadPaths.vue";
import FileBrowser from "./FileBrowser.vue";
import RepoDiffViewer from "./RepoDiffViewer.vue";
import { useLlamaHealth } from "../composables/useLlamaHealth";
import type { ExecutionMode, McpServerConfig, McpToolInfo, ProviderKind } from "../api";
import { READY_PROMPTS, type ReadyPrompt } from "../content/prompts";

const { t } = useI18n();

const fileBrowserVisible = ref(false);
const repoDiffVisible = ref(false);

// Modal de configuração do modo Long Horizon DESTA sessão (ícone de
// engrenagem, só aparece com o modo ligado) — memória/projeto editáveis,
// carregados na abertura e salvos por botão explícito (não a cada tecla,
// pra não gravar em disco a cada caractere digitado).
const longHorizonSettingsVisible = ref(false);
const longHorizonActiveTab = ref<"memoria" | "projeto" | "tarefas">("memoria");
const longHorizonMemoriaContent = ref("");
const longHorizonProjetoContent = ref("");
const longHorizonSettingsSaved = ref(false);

async function openLongHorizonSettings() {
  const id = sessionStore.currentId;
  if (!id) return;
  [longHorizonMemoriaContent.value, longHorizonProjetoContent.value, taskQueueJson.value] = await Promise.all([
    api.readSessionLongHorizonMemoria(id),
    api.readSessionLongHorizonProjeto(id),
    api.readSessionTaskQueue(id),
  ]);
  taskQueueSaveError.value = "";
  longHorizonActiveTab.value = "memoria";
  longHorizonSettingsVisible.value = true;
}

async function saveLongHorizonSettings() {
  const id = sessionStore.currentId;
  if (!id) return;
  await Promise.all([
    api.writeSessionLongHorizonMemoria(id, longHorizonMemoriaContent.value),
    api.writeSessionLongHorizonProjeto(id, longHorizonProjetoContent.value),
  ]);
  longHorizonSettingsSaved.value = true;
  setTimeout(() => (longHorizonSettingsSaved.value = false), 2000);
}

// Aba "Tarefas" do mesmo modal — JSON colado pelo usuário (convenção
// `{"tarefas": [...]}`), validado no backend antes de gravar (ver
// `write_session_task_queue`/`task_queue::parse_tasks`).
const taskQueueJson = ref("");
const taskQueueSaveError = ref("");

async function saveTaskQueue() {
  const id = sessionStore.currentId;
  if (!id) return;
  try {
    await api.writeSessionTaskQueue(id, taskQueueJson.value);
    taskQueueSaveError.value = "";
    longHorizonSettingsSaved.value = true;
    setTimeout(() => (longHorizonSettingsSaved.value = false), 2000);
  } catch (e) {
    taskQueueSaveError.value = String(e);
  }
}

const taskQueueRunning = computed(() => !!sessionStore.currentId && sessionStore.taskQueueRunningIds.has(sessionStore.currentId));

const taskQueueCanStart = computed(() => {
  const s = sessionStore.currentSession;
  return !!s && s.execution_mode === "yolo" && s.long_horizon.enabled;
});

const taskQueueStartTooltip = computed(() =>
  taskQueueCanStart.value ? t("composer.taskQueueStart") : t("composer.taskQueueRequirements"),
);

async function toggleTaskQueue() {
  if (taskQueueRunning.value) {
    await sessionStore.stopTaskQueue();
  } else {
    if (!taskQueueCanStart.value) return;
    await sessionStore.startTaskQueue();
  }
}

// "+" virou um menu (Popover) agrupando anexar arquivo/persona/Método
// Fable/MCP — pedido do usuário (2026-08-20), esses ficavam soltos no
// rodapé do composer poluindo a barra. Mesmo padrão de Popover já usado em
// ExtraReadPaths.vue.
const plusMenuRef = ref<InstanceType<typeof Popover> | null>(null);
function togglePlusMenu(event: Event) {
  plusMenuRef.value?.toggle(event);
}

const mcpServers = ref<McpServerConfig[]>([]);
async function loadMcpServers() {
  try {
    mcpServers.value = (await api.listMcpServers()).filter((s) => s.enabled);
  } catch {
    mcpServers.value = [];
  }
}
loadMcpServers();

const PERSONA_NONE = "__none__";

function onPersonaChange(value: string) {
  sessionStore.updatePersona(value === PERSONA_NONE ? null : value);
}

const enabledMcpNames = computed(() => {
  const session = sessionStore.currentSession;
  if (!session?.enabled_mcp_servers) return mcpServers.value.map((s) => s.name);
  return session.enabled_mcp_servers;
});

function toggleMcpServer(name: string) {
  const current = new Set(enabledMcpNames.value);
  if (current.has(name)) {
    current.delete(name);
  } else {
    current.add(name);
  }
  const allEnabled = mcpServers.value.every((s) => current.has(s.name));
  sessionStore.updateMcpServers(allEnabled ? null : [...current]);
}

// Fase E3: um único botão "MCPs" abrindo um modal com checkboxes, em vez de
// um botão por servidor lotando o rodapé do composer quando há muitos
// configurados.
const mcpModalVisible = ref(false);
const enabledMcpCount = computed(() => enabledMcpNames.value.length);

// Pedido do usuário testando ao vivo, 2026-08-17: ver quais ferramentas um
// MCP da lista realmente oferece (nome + descrição, quando o servidor
// fornece uma), sem precisar adivinhar pelo nome do servidor sozinho.
// Reaproveita `api.testMcpServer` (mesma chamada que Configurações já usa
// pra testar um servidor antes de salvar) — conexão descartável, não entra
// no pool compartilhado.
const mcpToolsModalVisible = ref(false);
const mcpToolsModalServerName = ref("");
const mcpToolsList = ref<McpToolInfo[]>([]);
const mcpToolsLoading = ref(false);
const mcpToolsError = ref("");

async function showMcpTools(server: McpServerConfig) {
  mcpToolsModalServerName.value = server.name;
  mcpToolsModalVisible.value = true;
  mcpToolsLoading.value = true;
  mcpToolsError.value = "";
  mcpToolsList.value = [];
  try {
    mcpToolsList.value = await api.listMcpServerTools(server);
  } catch (e) {
    mcpToolsError.value = String(e);
  } finally {
    mcpToolsLoading.value = false;
  }
}

const EXECUTION_MODE_OPTIONS = computed<{ value: ExecutionMode; label: string }[]>(() => [
  { value: "yolo", label: "⚡ YOLO" },
  { value: "auto", label: t("composer.modeAuto") },
  { value: "manual", label: t("composer.modeManual") },
]);

// Providers locais (llama.cpp/Ollama/LM Studio) não têm graduação real de
// esforço: o llama.cpp e derivados só entendem raciocínio como liga/desliga
// (chat_template_kwargs.enable_thinking, ver apply_reasoning no backend).
// "Baixo/Médio/Alto" contra esses providers eram um placebo — o campo
// reasoning_effort que a API OpenAI-compat usa pra graduar não é lido por
// nenhum dos forks/engines locais testados (TurboQuant, ik_llama.cpp,
// mainline). Providers de API (Openrouter/Custom, que cobre OpenRouter real,
// ChatGPT, Gemini etc.) têm graduação de verdade, então mantêm as 3 opções.
const LOCAL_PROVIDER_KINDS = new Set<ProviderKind>(["llama_cpp", "ollama", "lm_studio"]);

const isLocalProvider = computed(() => {
  const kind = sessionStore.currentSession?.provider;
  return kind ? LOCAL_PROVIDER_KINDS.has(kind) : false;
});

const REASONING_EFFORT_OPTIONS = computed<{
  value: "off" | "on" | "auto" | "low" | "medium" | "high";
  label: string;
}[]>(() => {
  if (isLocalProvider.value) {
    return [
      // Desligado é o default pra modelos locais: "Auto" deixaria o Qwen3/GLM
      // pensar por conta própria e ficar lento à toa.
      { value: "off", label: `💤 ${t("composer.reasoningOff")}` },
      { value: "on", label: `🧠 ${t("composer.reasoningOn")}` },
    ];
  }
  return [
    { value: "off", label: `💤 ${t("composer.reasoningOff")}` },
    { value: "auto", label: `🧠 ${t("composer.reasoningAuto")}` },
    { value: "low", label: `🧠 ${t("composer.reasoningLow")}` },
    { value: "medium", label: `🧠 ${t("composer.reasoningMedium")}` },
    { value: "high", label: `🧠 ${t("composer.reasoningHigh")}` },
  ];
});

function onExecutionModeChange(mode: ExecutionMode) {
  sessionStore.updateExecutionMode(mode);
}

const reasoningEffort = computed(
  () => sessionStore.currentSession?.reasoning_effort ?? "off",
);

function onReasoningEffortChange(value: "off" | "on" | "auto" | "low" | "medium" | "high") {
  sessionStore.updateReasoningEffort(value === "auto" ? null : value);
}

// Resumo em texto (modelo/modo/raciocínio) que substitui os dois dropdowns
// que ficavam sempre visíveis no rodapé — pedido do usuário (2026-08-20,
// inspirado no Claude Code desktop). Os dropdowns de verdade continuam
// existindo dentro do menu "+"; isso aqui só reflete o valor atual.
const pendingModelLabel = computed(() => {
  if (!pendingModel.value) return null;
  const list = providerStore.modelsFor(pendingProvider.value, pendingFork.value ?? undefined, pendingCustomProviderId.value ?? undefined);
  return list.find((m) => m.id === pendingModel.value)?.label ?? pendingModel.value;
});

const executionModeLabel = computed(() => {
  const mode = sessionStore.currentSession?.execution_mode;
  if (mode === "yolo") return "YOLO";
  if (mode === "auto") return t("composer.modeAuto");
  return t("composer.modeManual");
});

const reasoningLabel = computed(() => {
  const labels: Record<string, string> = {
    off: t("composer.reasoningOff"),
    on: t("composer.reasoningOn"),
    auto: t("composer.reasoningAuto"),
    low: t("composer.reasoningLow"),
    medium: t("composer.reasoningMedium"),
    high: t("composer.reasoningHigh"),
  };
  return labels[reasoningEffort.value] ?? reasoningEffort.value;
});

const composerSummary = computed(() =>
  t("composer.summary", {
    model: pendingModelLabel.value ?? t("composer.summaryNoModel"),
    mode: executionModeLabel.value,
    reasoning: reasoningLabel.value,
  }),
);

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
const providerStore = useProviderStore();

// Fase E2: catálogo de skills pro menu `/` — carregado aqui (reage a troca
// de pasta/sessão) mas escrito em `sessionStore.skills`, compartilhado com
// `AgentsSkillsPanel.vue`. Antes cada um tinha sua PRÓPRIA cópia local, e
// uma skill criada num painel nunca aparecia no outro sem recarregar a
// sessão inteira (bug real encontrado testando ao vivo, 2026-08-16, mesmo
// problema que a Persona abaixo tinha).
watch(
  () => sessionStore.currentSession?.project_root,
  (projectRoot) => sessionStore.loadSkills(projectRoot ?? null),
  { immediate: true },
);

const personaOptions = computed(() => [
  { value: PERSONA_NONE, label: t("composer.noPersona") },
  ...sessionStore.personas.map((p) => ({ value: p.id, label: p.name })),
]);

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
const DOCUMENT_EXTENSIONS = [
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
];

function attachmentFilters() {
  return [
    { name: t("composer.documentsAndCode"), extensions: DOCUMENT_EXTENSIONS },
    { name: t("composer.images"), extensions: IMAGE_EXTENSIONS },
  ];
}

// Fase E4: cache client-side do resultado de checkVisionSupport, por modelo
// (nao por sessao) — sem isso, toda vez que o usuario troca de sessao com o
// MESMO modelo o Cerne testaria visao de novo à toa (chamada extra que
// custa uma requisicao real ao provider/servidor local). Chave inclui
// fork/custom_provider_id porque o MESMO nome de modelo pode se comportar
// diferente entre dois forks locais ou dois providers customizados.
const VISION_CACHE_KEY = "cerne-vision-support-cache";
type VisionState = "untested" | "supported" | "unsupported";
const visionState = ref<VisionState>("untested");

function loadVisionCache(): Record<string, boolean> {
  try {
    const raw = localStorage.getItem(VISION_CACHE_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch {
    return {};
  }
}

function saveVisionCache(cache: Record<string, boolean>) {
  try {
    localStorage.setItem(VISION_CACHE_KEY, JSON.stringify(cache));
  } catch {
    // localStorage indisponivel (modo privado, quota) - cache vira só em memória pra essa sessão do app.
  }
}

function visionCacheKey() {
  const session = sessionStore.currentSession;
  if (!session) return null;
  return `${session.provider}::${session.model}::${session.llama_fork ?? session.custom_provider_id ?? ""}`;
}

async function refreshVisionSupport(force = false) {
  const sessionId = sessionStore.currentId;
  const key = visionCacheKey();
  if (!sessionId || !key) {
    visionSupported.value = false;
    visionState.value = "untested";
    return;
  }
  const cache = loadVisionCache();
  if (!force && key in cache) {
    visionSupported.value = cache[key];
    visionState.value = cache[key] ? "supported" : "unsupported";
    return;
  }
  visionState.value = "untested";
  try {
    const supported = await api.checkVisionSupport(sessionId);
    visionSupported.value = supported;
    visionState.value = supported ? "supported" : "unsupported";
    cache[key] = supported;
    saveVisionCache(cache);
  } catch {
    visionSupported.value = false;
    visionState.value = "unsupported";
  }
}

watch(() => sessionStore.currentSession, () => refreshVisionSupport(false), { immediate: true });

function retestVisionSupport() {
  refreshVisionSupport(true);
}

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
      error: t("composer.visionUnsupported"),
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
  const selected = await open({ directory: false, multiple: true, filters: attachmentFilters() });
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
async function onPaste(e: ClipboardEvent) {
  const items = e.clipboardData?.items;
  if (!items) return;

  // Detectar se o texto colado é um caminho de pasta válido
  const plainText = e.clipboardData?.getData("text/plain")?.trim();
  if (plainText && looksLikeFolderPath(plainText)) {
    const isDir = await api.checkPathIsDirectory(plainText).catch(() => false);
    if (isDir) {
      e.preventDefault();
      const current = sessionStore.currentSession?.extra_read_paths ?? [];
      if (current.some((entry) => entry.path === plainText)) return;
      // Perguntar ao usuário o modo de acesso
      const mode = window.confirm(
        `"${plainText}"\n\nEsta pasta será adicionada com permissão de escrita (leitura + escrita).\n\nOK = Leitura + Escrita\nCancelar = Só Leitura`,
      ) ? "read_write" as const : "read" as const;
      await sessionStore.updateExtraReadPaths([...current, { path: plainText, mode }]);
      return;
    }
  }

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
    reader.onload = async () => {
      const cru = reader.result as string;
      // Otimiza no backend antes de guardar: redimensiona ao mesmo limite e
      // recomprime em JPEG, igual ao caminho de anexar arquivo (ver
      // `image_util` no Rust). Um print de tela cheia cai de ~2.765 pra ~1.229
      // tokens (e o base64 de vários MB vira dezenas de KB). Se a otimização
      // falhar, o backend devolve o original — a imagem nunca se perde.
      const dataUrl = await api.optimizeImageDataUrl(cru).catch(() => cru);
      updateAttachment(id, { status: "ready", dataUrl });
    };
    reader.onerror = () => updateAttachment(id, { status: "error", error: t("composer.pasteImageFailed") });
    reader.readAsDataURL(file);
  }
}

function looksLikeFolderPath(text: string): boolean {
  // Caminho Windows: C:\..., D:\..., \\server\share
  // Caminho Unix: /home/..., /usr/...
  if (/^[A-Za-z]:[\\/]/.test(text) || /^\\\\/.test(text) || /^\//.test(text)) {
    return !text.includes("\n") && text.length < 500;
  }
  return false;
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
  providerStore.setActiveSelection(pendingProvider.value, id, pendingFork.value ?? undefined, pendingCustomProviderId.value ?? undefined);
}

// Voz — microfone no composer (STT via OpenRouter). Escopo reduzido a
// pedido do usuário: só grava e transcreve pro texto do composer, NUNCA
// envia sozinho — o usuário revisa/edita e manda como qualquer mensagem
// normal, igual o padrão já usado pro import de skill (nunca "faz sozinho").
type MicState = "idle" | "recording" | "transcribing" | "error";
const micState = ref<MicState>("idle");
let mediaRecorder: MediaRecorder | null = null;
let recordedChunks: Blob[] = [];

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onloadend = () => {
      const result = reader.result as string;
      // FileReader.readAsDataURL devolve "data:<mime>;base64,<dados>" —
      // só a parte depois da vírgula interessa pro backend.
      resolve(result.split(",")[1] ?? "");
    };
    reader.onerror = reject;
    reader.readAsDataURL(blob);
  });
}

async function toggleMic() {
  if (micState.value === "recording") {
    mediaRecorder?.stop();
    return;
  }
  if (micState.value === "transcribing") return;
  try {
    const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
    recordedChunks = [];
    mediaRecorder = new MediaRecorder(stream);
    mediaRecorder.ondataavailable = (e) => {
      if (e.data.size > 0) recordedChunks.push(e.data);
    };
    mediaRecorder.onstop = async () => {
      stream.getTracks().forEach((track) => track.stop());
      micState.value = "transcribing";
      try {
        const blob = new Blob(recordedChunks, { type: "audio/webm" });
        const base64 = await blobToBase64(blob);
        const transcribed = await api.sttTranscribe(base64, "webm");
        if (transcribed.trim()) {
          text.value = text.value.trim() ? `${text.value.trim()} ${transcribed.trim()}` : transcribed.trim();
        }
        micState.value = "idle";
      } catch {
        micState.value = "error";
        setTimeout(() => (micState.value = "idle"), 2500);
      }
    };
    mediaRecorder.start();
    micState.value = "recording";
  } catch {
    micState.value = "error";
    setTimeout(() => (micState.value = "idle"), 2500);
  }
}

const healthTargetFork = computed(() => (pendingProvider.value === "llama_cpp" ? pendingFork.value : null));
const { isUp: llamaIsUp } = useLlamaHealth(healthTargetFork);

function grow() {
  const el = textareaRef.value;
  if (!el) return;
  el.style.height = "auto";
  el.style.height = Math.min(el.scrollHeight, 240) + "px";
}

// Fase E2: atalho `/` no composer — abre um popover só quando `/` é o
// PRIMEIRO caractere digitado numa caixa vazia (nunca no meio de texto já
// digitado, pra não atrapalhar quem cola um caminho tipo "/home/user/...").
// Fecha sozinho assim que aparece um espaço/quebra de linha (deixa de ser
// atalho, vira texto normal) ou quando uma opção é escolhida.
interface SlashItem {
  kind: "skill" | "persona" | "mcp" | "prompt" | "pipeline";
  key: string;
  label: string;
  sublabel: string;
  icon: string;
}

const slashMenuOpen = ref(false);
const slashActiveIndex = ref(0);

const slashQuery = computed(() => (text.value.startsWith("/") ? text.value.slice(1) : ""));

const slashHasProject = computed(() => !!sessionStore.currentSession?.project_root);

function readyPromptText(p: ReadyPrompt, field: "title" | "toolsLabel" | "full"): string {
  return t(`readyPrompts.${p.id}.${field}`);
}

const slashAllItems = computed<SlashItem[]>(() => {
  const items: SlashItem[] = [];
  for (const skill of sessionStore.skills) {
    items.push({
      kind: "skill",
      key: skill.dir,
      label: skill.name,
      sublabel: skill.description,
      icon: "auto_awesome",
    });
  }
  for (const persona of sessionStore.personas) {
    items.push({
      kind: "persona",
      key: persona.id,
      label: persona.name,
      sublabel: t("composer.slashPersonaSublabel"),
      icon: "person",
    });
  }
  for (const srv of mcpServers.value) {
    items.push({
      kind: "mcp",
      key: srv.name,
      label: srv.name,
      sublabel: enabledMcpNames.value.includes(srv.name)
        ? t("composer.slashMcpEnabledSublabel")
        : t("composer.slashMcpDisabledSublabel"),
      icon: "extension",
    });
  }
  for (const prompt of READY_PROMPTS) {
    if (prompt.scope === "code" && !slashHasProject.value) continue;
    if (prompt.scope === "chat" && slashHasProject.value) continue;
    items.push({
      kind: "prompt",
      key: prompt.id,
      label: readyPromptText(prompt, "title"),
      sublabel: readyPromptText(prompt, "toolsLabel"),
      icon: "bolt",
    });
  }
  // Fase 3: pipeline Dev→QA→Analista — só faz sentido com pasta de projeto
  // associada (run_pipeline exige project_root, mesma restrição de task/
  // verify_completion).
  if (slashHasProject.value) {
    items.push({
      kind: "pipeline",
      key: "pipeline",
      label: t("composer.slashPipelineLabel"),
      sublabel: t("composer.slashPipelineSublabel"),
      icon: "conversion_path",
    });
  }
  return items;
});

const slashItems = computed(() => {
  const q = slashQuery.value.toLowerCase().trim();
  if (!q) return slashAllItems.value;
  return slashAllItems.value.filter(
    (item) => item.label.toLowerCase().includes(q) || item.sublabel.toLowerCase().includes(q),
  );
});

watch(slashItems, () => {
  slashActiveIndex.value = 0;
});

function onComposerInput() {
  grow();
  const value = text.value;
  if (value === "/") {
    slashMenuOpen.value = true;
    slashActiveIndex.value = 0;
  } else if (slashMenuOpen.value && (!value.startsWith("/") || /\s/.test(value))) {
    slashMenuOpen.value = false;
  }
}

function selectSlashItem(item: SlashItem) {
  if (item.kind === "skill") {
    text.value = t("agentsSkillsPanel.useSkillDraft", { name: item.label });
  } else if (item.kind === "persona") {
    onPersonaChange(item.key);
    text.value = "";
  } else if (item.kind === "mcp") {
    toggleMcpServer(item.key);
    text.value = "";
  } else if (item.kind === "pipeline") {
    // Diferente de skill/persona (ação imediata ou pedido genérico), aqui o
    // usuário ainda precisa descrever o requisito — só prepara a frase e
    // deixa o cursor pronto pra continuar digitando.
    text.value = t("composer.slashPipelineDraft");
  } else {
    const prompt = READY_PROMPTS.find((p) => p.id === item.key);
    text.value = prompt ? readyPromptText(prompt, "full") : "";
  }
  slashMenuOpen.value = false;
  nextTick(() => {
    grow();
    textareaRef.value?.focus();
  });
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
  if (slashMenuOpen.value) {
    if (e.key === "ArrowDown") {
      e.preventDefault();
      slashActiveIndex.value = Math.min(slashActiveIndex.value + 1, slashItems.value.length - 1);
      return;
    }
    if (e.key === "ArrowUp") {
      e.preventDefault();
      slashActiveIndex.value = Math.max(slashActiveIndex.value - 1, 0);
      return;
    }
    if (e.key === "Enter") {
      e.preventDefault();
      const item = slashItems.value[slashActiveIndex.value];
      if (item) selectSlashItem(item);
      return;
    }
    if (e.key === "Escape") {
      e.preventDefault();
      slashMenuOpen.value = false;
      return;
    }
  }
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
    <!-- `defer` é obrigatório aqui (Vue 3.5+): o alvo (#chat-topbar-left,
         renderizado por ChatView.vue) monta na MESMA passada síncrona que
         este componente — sem `defer` o Vue tenta resolver o seletor antes
         do alvo existir no DOM e falha silenciosamente (achado testando ao
         vivo, 2026-08-20: "Failed to locate Teleport target" no console,
         topo do chat ficava vazio). -->
    <Teleport to="#chat-topbar-left" defer>
      <StatusDot v-if="pendingProvider === 'llama_cpp'" :up="llamaIsUp" />
      <ProviderPicker
        hide-vision-test
        :provider="pendingProvider"
        :fork="pendingFork"
        :custom-provider-id="pendingCustomProviderId"
        :model="pendingModel"
        @update:provider="onProviderChange"
        @update:fork="onForkChange"
        @update:custom-provider-id="onCustomProviderIdChange"
        @update:model="onModelChange"
      />
      <button
        v-if="sessionStore.currentSession"
        class="vision-icon-btn"
        :class="`vision-${visionState}`"
        v-tooltip.top="$t(`composer.visionState.${visionState}`)"
        @click="retestVisionSupport"
      >
        <span class="msi">{{ visionState === "unsupported" ? "visibility_off" : "visibility" }}</span>
      </button>
    </Teleport>
    <Teleport to="#chat-topbar-right" defer>
      <button
        v-if="sessionStore.currentSession"
        class="file-browser-btn"
        v-tooltip.top="$t('fileBrowser.tooltip')"
        @click="fileBrowserVisible = true"
      >
        <span class="msi">folder_open</span>
      </button>
      <button
        v-if="sessionStore.currentSession"
        class="file-browser-btn"
        v-tooltip.top="$t('repoDiffViewer.tooltip')"
        @click="repoDiffVisible = true"
      >
        <span class="msi">difference</span>
      </button>
    </Teleport>
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
        <span class="attachment-name">{{ savingAttachments && a.kind === 'document' && !a.savedMdPath ? $t("composer.optimizing") : a.saved ? $t("composer.ready") : a.name }}</span>
        <button class="attachment-remove" v-tooltip.top="$t('composer.remove')" @click="removeAttachment(a.id)" :disabled="savingAttachments">
          <span class="msi">close</span>
        </button>
      </div>
    </div>
    <div v-if="savingAttachments" class="saving-hint">
      <span class="msi spin">progress_activity</span>
      {{ $t("composer.optimizingHint") }}
    </div>
    <div class="composer-input-wrap">
      <textarea
        ref="textareaRef"
        v-model="text"
        class="composer-input"
        rows="1"
        :placeholder="$t('composer.placeholder')"
        @input="onComposerInput"
        @keydown="onKeydown"
        @paste="onPaste"
        @blur="slashMenuOpen = false"
      />
      <div v-if="slashMenuOpen" class="slash-menu">
        <button
          v-for="(item, idx) in slashItems"
          :key="`${item.kind}-${item.key}`"
          class="slash-item"
          :class="{ active: idx === slashActiveIndex }"
          @mousedown.prevent="selectSlashItem(item)"
          @mouseenter="slashActiveIndex = idx"
        >
          <span class="msi slash-item-icon">{{ item.icon }}</span>
          <span class="slash-item-text">
            <span class="slash-item-label">{{ item.label }}</span>
            <span class="slash-item-sublabel">{{ item.sublabel }}</span>
          </span>
          <span class="slash-item-kind">{{ $t(`composer.slashKind.${item.kind}`) }}</span>
        </button>
        <p v-if="slashItems.length === 0" class="slash-empty">{{ $t("composer.slashEmpty") }}</p>
      </div>
    </div>
    <div class="composer-footer">
      <div class="footer-left">
        <button class="attach-btn" v-tooltip.top="$t('composer.plusMenuTooltip')" @click="togglePlusMenu">
          <span class="msi">add</span>
        </button>
        <button
          v-if="sessionStore.currentSession?.long_horizon.enabled"
          class="attach-btn"
          v-tooltip.top="$t('composer.longHorizonSettingsTooltip')"
          @click="openLongHorizonSettings"
        >
          <span class="msi">settings</span>
        </button>
        <button
          v-if="sessionStore.currentSession?.task_queue_enabled"
          class="attach-btn"
          :class="{ 'task-queue-running': taskQueueRunning }"
          :disabled="!taskQueueRunning && !taskQueueCanStart"
          v-tooltip.top="taskQueueRunning ? $t('composer.taskQueueStop') : taskQueueStartTooltip"
          @click="toggleTaskQueue"
        >
          <span class="msi">{{ taskQueueRunning ? "stop" : "play_arrow" }}</span>
        </button>
        <Popover ref="plusMenuRef">
          <div class="plus-menu">
            <button class="plus-menu-item" @click="addAttachments(); plusMenuRef?.hide()">
              <span class="msi">attach_file</span>
              <span class="plus-menu-label">{{ $t("composer.attachFile") }}</span>
            </button>
            <ExtraReadPaths v-if="sessionStore.currentSession" />
            <div v-if="sessionStore.currentSession" class="plus-menu-item plus-menu-row">
              <span class="msi">tune</span>
              <span class="plus-menu-label">{{ $t("composer.executionModeLabel") }}</span>
              <Select
                :modelValue="sessionStore.currentSession.execution_mode"
                @update:modelValue="(v) => onExecutionModeChange(v as ExecutionMode)"
                :options="EXECUTION_MODE_OPTIONS"
                optionLabel="label"
                optionValue="value"
                class="execution-mode-select"
                size="small"
              />
            </div>
            <div v-if="sessionStore.currentSession" class="plus-menu-item plus-menu-row">
              <span class="msi">psychology</span>
              <span class="plus-menu-label">{{ $t("composer.reasoningLabel") }}</span>
              <Select
                :modelValue="reasoningEffort"
                @update:modelValue="(v) => onReasoningEffortChange(v as 'off' | 'on' | 'auto' | 'low' | 'medium' | 'high')"
                :options="REASONING_EFFORT_OPTIONS"
                optionLabel="label"
                optionValue="value"
                class="execution-mode-select"
                size="small"
              />
            </div>
            <div v-if="sessionStore.currentSession && sessionStore.personas.length > 0" class="plus-menu-item plus-menu-row">
              <span class="msi">theater_comedy</span>
              <span class="plus-menu-label">{{ $t("composer.personaLabel") }}</span>
              <Select
                :modelValue="sessionStore.currentSession.persona_id ?? PERSONA_NONE"
                @update:modelValue="(v) => onPersonaChange(v as string)"
                :options="personaOptions"
                optionLabel="label"
                optionValue="value"
                class="execution-mode-select persona-select"
                size="small"
              />
            </div>
            <button
              v-if="sessionStore.currentSession"
              class="plus-menu-item plus-menu-toggle"
              :class="{ 'plus-menu-toggle-on': sessionStore.currentSession.fable_method }"
              v-tooltip.right="$t('composer.fableTooltip')"
              @click="sessionStore.updateFableMethod(!sessionStore.currentSession.fable_method)"
            >
              <span class="msi">route</span>
              <span class="plus-menu-label">{{ $t("composer.fableLabel") }}</span>
              <span class="plus-menu-switch"><span class="plus-menu-switch-dot" /></span>
            </button>
            <button
              v-if="sessionStore.currentSession"
              class="plus-menu-item plus-menu-toggle"
              :class="{ 'plus-menu-toggle-on': sessionStore.currentSession.long_horizon.enabled }"
              v-tooltip.right="$t('composer.longHorizonTooltip')"
              @click="sessionStore.updateLongHorizonEnabled(!sessionStore.currentSession.long_horizon.enabled)"
            >
              <span class="msi">all_inclusive</span>
              <span class="plus-menu-label">{{ $t("composer.longHorizonLabel") }}</span>
              <span class="plus-menu-switch"><span class="plus-menu-switch-dot" /></span>
            </button>
            <button
              v-if="sessionStore.currentSession"
              class="plus-menu-item plus-menu-toggle"
              :class="{ 'plus-menu-toggle-on': sessionStore.currentSession.task_queue_enabled }"
              v-tooltip.right="$t('composer.taskQueueTooltip')"
              @click="sessionStore.updateTaskQueueEnabled(!sessionStore.currentSession.task_queue_enabled)"
            >
              <span class="msi">playlist_play</span>
              <span class="plus-menu-label">{{ $t("composer.taskQueueLabel") }}</span>
              <span class="plus-menu-switch"><span class="plus-menu-switch-dot" /></span>
            </button>
            <button
              v-if="mcpServers.length > 0"
              class="plus-menu-item"
              @click="mcpModalVisible = true; plusMenuRef?.hide()"
            >
              <span class="msi">extension</span>
              <span class="plus-menu-label">{{ $t("composer.mcpLabel") }}</span>
              <span class="plus-menu-badge">{{ enabledMcpCount }}/{{ mcpServers.length }}</span>
            </button>
          </div>
        </Popover>
      </div>
      <div class="footer-right">
        <ContextGauge />
        <button
          v-if="sessionStore.status === 'idle'"
          class="mic-btn"
          :class="{ recording: micState === 'recording', error: micState === 'error' }"
          :disabled="micState === 'transcribing'"
          v-tooltip.top="$t(micState === 'recording' ? 'composer.micStop' : 'composer.micStart')"
          @click="toggleMic"
        >
          <span class="msi">{{ micState === "transcribing" ? "hourglass_top" : micState === "error" ? "error" : micState === "recording" ? "stop" : "mic" }}</span>
        </button>
        <button
          v-if="sessionStore.status === 'idle'"
          class="send-btn"
          :disabled="!text.trim() || savingAttachments"
          @click="submit"
        >
          <span class="msi">arrow_upward</span>
        </button>
        <button v-else class="send-btn stop-btn" v-tooltip.top="$t('composer.cancelExecution')" @click="sessionStore.cancelTurn()">
          <span class="msi">stop</span>
        </button>
      </div>
    </div>
    <FileBrowser v-model:visible="fileBrowserVisible" />
    <RepoDiffViewer v-model:visible="repoDiffVisible" />

    <Dialog
      v-model:visible="mcpModalVisible"
      :header="$t('composer.mcpModalTitle')"
      modal
      :style="{ width: 'min(480px, 92vw)' }"
    >
      <div class="mcp-modal-list">
        <div v-for="srv in mcpServers" :key="srv.name" class="mcp-modal-item">
          <label v-tooltip.right="`${srv.command} ${srv.args.join(' ')}`">
            <input
              type="checkbox"
              :checked="enabledMcpNames.includes(srv.name)"
              @change="toggleMcpServer(srv.name)"
            />
            <span class="mcp-modal-name">{{ srv.name }}</span>
          </label>
          <button
            class="mcp-tools-btn"
            v-tooltip.top="$t('composer.mcpViewTools')"
            @click="showMcpTools(srv)"
          >
            <span class="msi">list_alt</span>
          </button>
        </div>
        <p v-if="mcpServers.length === 0" class="mcp-modal-empty">{{ $t("composer.mcpModalEmpty") }}</p>
      </div>
    </Dialog>

    <Dialog
      v-model:visible="mcpToolsModalVisible"
      :header="mcpToolsModalServerName"
      modal
      :style="{ width: 'min(480px, 92vw)' }"
    >
      <p v-if="mcpToolsLoading" class="hint">{{ $t("composer.mcpToolsLoading") }}</p>
      <p v-else-if="mcpToolsError" class="error-text">{{ mcpToolsError }}</p>
      <div v-else-if="mcpToolsList.length > 0" class="mcp-tools-list">
        <div v-for="tool in mcpToolsList" :key="tool.name" class="mcp-tools-item">
          <span class="mcp-tools-name">{{ tool.name }}</span>
          <p v-if="tool.description" class="mcp-tools-desc">{{ tool.description }}</p>
        </div>
      </div>
      <p v-else class="hint">{{ $t("composer.mcpToolsEmpty") }}</p>
    </Dialog>

    <Dialog
      v-model:visible="longHorizonSettingsVisible"
      modal
      :style="{ width: 'min(820px, 94vw)' }"
      class="lh-dialog"
    >
      <div class="lh">
        <aside class="lh-nav">
          <p class="lh-nav-label">{{ $t("composer.longHorizonSettingsTitle") }}</p>
          <button
            type="button"
            class="lh-nav-item"
            :class="{ active: longHorizonActiveTab === 'memoria' }"
            @click="longHorizonActiveTab = 'memoria'"
          >
            <span class="msi">psychology</span>
            <span class="lh-nav-text">{{ $t("composer.longHorizonMemoriaLabel") }}</span>
          </button>
          <button
            type="button"
            class="lh-nav-item"
            :class="{ active: longHorizonActiveTab === 'projeto' }"
            @click="longHorizonActiveTab = 'projeto'"
          >
            <span class="msi">checklist</span>
            <span class="lh-nav-text">{{ $t("composer.longHorizonProjetoLabel") }}</span>
          </button>
          <button
            type="button"
            class="lh-nav-item"
            :class="{ active: longHorizonActiveTab === 'tarefas' }"
            @click="longHorizonActiveTab = 'tarefas'"
          >
            <span class="msi">playlist_play</span>
            <span class="lh-nav-text">{{ $t("composer.taskQueueTabLabel") }}</span>
          </button>
          <p class="lh-nav-status">
            {{ $t("composer.longHorizonSettingsHint", {
              iteracao: sessionStore.currentSession?.long_horizon.iteracao_atual ?? 0,
              desfecho: sessionStore.currentSession?.long_horizon.ultimo_desfecho ?? $t('composer.longHorizonNoOutcome'),
            }) }}
          </p>
        </aside>

        <div class="lh-main">
          <header class="lh-head">
            <h2 class="lh-head-title">
              {{ longHorizonActiveTab === "memoria"
                ? $t("composer.longHorizonMemoriaLabel")
                : longHorizonActiveTab === "projeto"
                ? $t("composer.longHorizonProjetoLabel")
                : $t("composer.taskQueueTabLabel") }}
            </h2>
            <button
              type="button"
              class="lh-close"
              :aria-label="$t('settings.close')"
              @click="longHorizonSettingsVisible = false"
            >
              <span class="msi">close</span>
            </button>
          </header>
          <div class="lh-body">
            <textarea
              v-show="longHorizonActiveTab === 'memoria'"
              v-model="longHorizonMemoriaContent"
              class="text-input lh-textarea"
              :placeholder="$t('composer.longHorizonMemoriaPlaceholder')"
            />
            <textarea
              v-show="longHorizonActiveTab === 'projeto'"
              v-model="longHorizonProjetoContent"
              class="text-input lh-textarea"
              :placeholder="$t('composer.longHorizonProjetoPlaceholder')"
            />
            <div v-if="longHorizonActiveTab === 'tarefas'" class="task-queue-tab">
              <p class="hint">
                {{ $t("composer.taskQueueTabHint") }}
                <code class="task-queue-format">{{ '{"tarefas": [{"id": 1, "descricao": "...", "status": "fazer"}]}' }}</code>
              </p>
              <textarea
                v-model="taskQueueJson"
                class="text-input lh-textarea task-queue-textarea"
                :placeholder="$t('composer.taskQueueTabPlaceholder')"
                spellcheck="false"
              />
              <p v-if="taskQueueSaveError" class="error-text">{{ taskQueueSaveError }}</p>
              <p v-if="!taskQueueCanStart" class="hint task-queue-req-hint">{{ $t("composer.taskQueueRequirements") }}</p>
            </div>
          </div>
          <div class="lh-footer">
            <template v-if="longHorizonActiveTab === 'tarefas'">
              <button class="btn-primary" @click="saveTaskQueue">{{ $t("sidebar.save") }}</button>
              <button
                class="btn-secondary"
                :class="{ 'task-queue-running': taskQueueRunning }"
                :disabled="!taskQueueRunning && !taskQueueCanStart"
                @click="toggleTaskQueue"
              >
                <span class="msi">{{ taskQueueRunning ? "stop" : "play_arrow" }}</span>
                {{ taskQueueRunning ? $t("composer.taskQueueStop") : $t("composer.taskQueueStart") }}
              </button>
            </template>
            <button v-else class="btn-primary" @click="saveLongHorizonSettings">{{ $t("sidebar.save") }}</button>
            <span v-if="longHorizonSettingsSaved" class="mcp-test-success">
              <span class="msi">check_circle</span>
              {{ $t("composer.longHorizonSaved") }}
            </span>
            <span v-if="longHorizonActiveTab === 'tarefas' && sessionStore.taskQueueLastEvent" class="task-queue-event" :class="sessionStore.taskQueueLastEvent.status">
              <span class="msi">{{ sessionStore.taskQueueLastEvent.status === "stuck" || sessionStore.taskQueueLastEvent.status === "error" ? "warning" : "info" }}</span>
              {{ sessionStore.taskQueueLastEvent.message ?? sessionStore.taskQueueLastEvent.status }}
            </span>
          </div>
        </div>
      </div>
    </Dialog>
  </div>
  <!-- Resumo em texto do que está selecionado (modelo/modo/raciocínio) —
       pedido do usuário (2026-08-20), inspirado no Claude Code desktop.
       FORA da caixa do composer (abaixo dela), não dentro do rodapé — os
       dropdowns de verdade (Modo/Raciocínio) vivem no menu "+". -->
  <span v-if="sessionStore.currentSession" class="composer-summary">{{ composerSummary }}</span>
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

.composer-input {
  width: 100%;
  border: none;
  outline: none;
  resize: none;
  font-size: var(--cerne-font-composer, 14px);
  font-weight: 500;
  font-family: inherit;
  color: #18181b;
  line-height: 1.5;
  max-height: 240px;
}

.composer-input::placeholder {
  color: #a1a1aa;
}

.composer-input-wrap {
  position: relative;
}

.slash-menu {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  right: 0;
  z-index: 20;
  background: #ffffff;
  border: var(--cerne-border);
  border-radius: 10px;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.14);
  padding: 4px;
  max-height: 280px;
  overflow-y: auto;
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.slash-item {
  display: flex;
  align-items: center;
  gap: 8px;
  border: none;
  background: transparent;
  border-radius: 6px;
  padding: 7px 8px;
  cursor: pointer;
  text-align: left;
  width: 100%;
  box-sizing: border-box;
}

.slash-item.active,
.slash-item:hover {
  background: #f4f4f5;
}

.slash-item-icon {
  font-size: 16px;
  color: #71717a;
  flex-shrink: 0;
}

.slash-item-text {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}

.slash-item-label {
  font-size: 13px;
  font-weight: 600;
  color: #18181b;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slash-item-sublabel {
  font-size: 11px;
  font-weight: 500;
  color: #a1a1aa;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.slash-item-kind {
  font-size: 10px;
  font-weight: 600;
  color: #a1a1aa;
  background: #f4f4f5;
  border-radius: 4px;
  padding: 2px 6px;
  flex-shrink: 0;
  text-transform: uppercase;
}

.slash-empty {
  font-size: 12px;
  font-weight: 500;
  color: #a1a1aa;
  padding: 8px;
  margin: 0;
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

.composer-summary {
  display: block;
  margin-top: 6px;
  padding: 0 4px;
  font-size: 11px;
  font-weight: 500;
  color: #a1a1aa;
  text-align: center;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
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

.persona-select {
  max-width: 140px;
}

.persona-select :deep(.p-select-label) {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
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

.file-browser-btn {
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

.file-browser-btn:hover {
  border-color: #d4d4d8;
}

.vision-icon-btn {
  border: var(--cerne-border);
  background: #ffffff;
  border-radius: 8px;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: #a1a1aa;
  flex-shrink: 0;
}

.vision-icon-btn .msi {
  font-size: 17px;
}

.vision-icon-btn.vision-untested {
  color: #a1a1aa;
}

.vision-icon-btn.vision-supported {
  border-color: #22c55e;
  color: #16a34a;
  background: #ecfdf3;
}

.vision-icon-btn.vision-unsupported {
  border-color: #f87171;
  color: #dc2626;
  background: #fef2f2;
}

.plus-menu {
  display: flex;
  flex-direction: column;
  gap: 2px;
  width: 240px;
}

.plus-menu-item {
  display: flex;
  align-items: center;
  gap: 8px;
  border: none;
  background: transparent;
  border-radius: 8px;
  padding: 8px;
  cursor: pointer;
  color: #3f3f46;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  text-align: left;
  width: 100%;
}

.plus-menu-item:hover {
  background: #f4f4f5;
}

.plus-menu-item .msi {
  font-size: 17px;
  color: #71717a;
  flex-shrink: 0;
}

.plus-menu-label {
  flex: 1;
  min-width: 0;
}

.plus-menu-row {
  cursor: default;
}

.plus-menu-row:hover {
  background: transparent;
}

.plus-menu-badge {
  font-size: 11px;
  font-weight: 700;
  color: #71717a;
  background: #f4f4f5;
  border-radius: 999px;
  padding: 2px 8px;
  flex-shrink: 0;
}

.plus-menu-switch {
  flex-shrink: 0;
  width: 30px;
  height: 18px;
  border-radius: 999px;
  background: #e4e4e7;
  position: relative;
  transition: background 0.15s;
}

.plus-menu-switch-dot {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: #ffffff;
  transition: transform 0.15s;
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
}

/* Ligado = trilho rosa, pra deixar óbvio que o método Fable está ativo
   (é opt-in, só faz sentido em modelos pequenos/médios). */
.plus-menu-toggle-on .plus-menu-switch {
  background: #db2777;
}

.plus-menu-toggle-on .plus-menu-switch-dot {
  transform: translateX(12px);
}

.mic-btn {
  border: var(--cerne-border);
  background: #ffffff;
  color: #a1a1aa;
  border-radius: 8px;
  width: 30px;
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  flex-shrink: 0;
}

.mic-btn:disabled {
  color: #d4d4d8;
  cursor: default;
}

.mic-btn.recording {
  border-color: #dc2626;
  color: #dc2626;
  background: #fef2f2;
}

.mic-btn.error {
  border-color: #f87171;
  color: #dc2626;
}

.mic-btn .msi {
  font-size: 17px;
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
.attach-btn .msi {
  font-size: 18px;
}

.mcp-modal-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
  max-height: 360px;
  overflow-y: auto;
}

.mcp-modal-item {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 2px 6px;
  border-radius: 6px;
}

.mcp-modal-item:hover {
  background: #f4f4f5;
}

.mcp-modal-item label {
  display: flex;
  align-items: center;
  gap: 8px;
  flex: 1;
  min-width: 0;
  padding: 6px 0;
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  color: #3f3f46;
}

.mcp-modal-name {
  font-family: var(--cerne-mono);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.mcp-modal-empty {
  font-size: 12px;
  font-weight: 500;
  color: #a1a1aa;
}

.mcp-tools-btn {
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: #a1a1aa;
  cursor: pointer;
  padding: 4px;
  border-radius: 6px;
}

.mcp-tools-btn:hover {
  background: #e4e4e7;
  color: #18181b;
}

.mcp-tools-btn .msi {
  font-size: 18px;
}

.mcp-tools-list {
  display: flex;
  flex-direction: column;
  gap: 10px;
  max-height: 400px;
  overflow-y: auto;
}

.mcp-tools-item {
  padding-bottom: 8px;
  border-bottom: 1px solid #f4f4f5;
}

.mcp-tools-item:last-child {
  border-bottom: none;
  padding-bottom: 0;
}

.mcp-tools-name {
  font-family: var(--cerne-mono);
  font-size: 12.5px;
  font-weight: 600;
  color: #18181b;
}

.mcp-tools-desc {
  margin: 3px 0 0;
  font-size: 12px;
  font-weight: 400;
  color: #71717a;
  overflow-wrap: break-word;
}

.hint {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
}

.error-text {
  font-size: 12px;
  font-weight: 500;
  color: #dc2626;
}

/* Modal "Long Horizon — esta sessão": mesmo layout de dois painéis
   (navegação à esquerda, conteúdo grande à direita) que Settings.vue usa —
   pedido do usuário (2026-09-20): duas textareas pequenas empilhadas
   ficavam apertadas, memória/projeto merecem o espaço inteiro cada uma. */
.lh-dialog :deep(.p-dialog-content) {
  padding: 0;
  overflow: hidden;
}

.lh {
  display: flex;
  height: min(560px, 76vh);
}

.lh-nav {
  flex: 0 0 200px;
  display: flex;
  flex-direction: column;
  border-right: var(--cerne-border);
  padding: 12px 10px 14px;
  min-width: 0;
}

.lh-nav-label {
  font-size: 10.5px;
  font-weight: 700;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: #a1a1aa;
  margin: 0 0 8px;
  padding: 0 8px;
}

.lh-nav-item {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  border: none;
  background: transparent;
  text-align: left;
  padding: 8px;
  border-radius: 6px;
  cursor: pointer;
  color: #3f3f46;
  font-family: inherit;
  font-size: 13px;
  font-weight: 500;
  flex-shrink: 0;
}

.lh-nav-item .msi {
  font-size: 18px;
  color: #a1a1aa;
  flex-shrink: 0;
}

.lh-nav-item:hover {
  background: #f4f4f5;
}

.lh-nav-item.active {
  background: #eef2ff;
  color: var(--cerne-accent, #6366f1);
  font-weight: 600;
}

.lh-nav-item.active .msi {
  color: var(--cerne-accent, #6366f1);
}

.lh-nav-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.lh-nav-status {
  margin-top: auto;
  padding: 8px;
  font-size: 11.5px;
  line-height: 1.5;
  color: #71717a;
}

.lh-main {
  flex: 1;
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.lh-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  padding: 14px 20px 12px;
  border-bottom: var(--cerne-border);
  flex-shrink: 0;
}

.lh-head-title {
  font-size: 15px;
  font-weight: 700;
  margin: 0;
  color: #18181b;
}

.btn-primary {
  border: none;
  background: #18181b;
  color: #ffffff;
  border-radius: 8px;
  padding: 7px 14px;
  font-size: 13px;
  font-weight: 600;
  cursor: pointer;
}

.mcp-test-success {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
  color: #16a34a;
}

.mcp-test-success .msi {
  font-size: 15px;
}

.lh-close {
  display: flex;
  align-items: center;
  justify-content: center;
  border: none;
  background: transparent;
  color: #a1a1aa;
  padding: 4px;
  border-radius: 6px;
  cursor: pointer;
  flex-shrink: 0;
}

.lh-close:hover {
  background: #f4f4f5;
  color: #18181b;
}

.lh-close .msi {
  font-size: 19px;
}

.lh-body {
  flex: 1;
  min-height: 0;
  padding: 16px 20px;
  display: flex;
}

.lh-textarea {
  flex: 1;
  width: 100%;
  height: 100%;
  resize: none;
  font-family: inherit;
  line-height: 1.6;
}

.task-queue-tab {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.task-queue-textarea {
  font-family: var(--cerne-mono, monospace);
  font-size: 12px;
}

.task-queue-req-hint {
  color: #b45309;
}

.task-queue-format {
  display: block;
  margin-top: 6px;
  font-family: var(--cerne-mono, monospace);
  font-size: 11px;
  background: rgba(0, 0, 0, 0.04);
  padding: 4px 6px;
  border-radius: 4px;
  word-break: break-all;
}

.task-queue-running .msi {
  color: #dc2626;
}

.task-queue-event {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  margin-left: auto;
}

.task-queue-event .msi {
  font-size: 15px;
}

.task-queue-event.stuck .msi,
.task-queue-event.error .msi {
  color: #dc2626;
}

.task-queue-event.finished .msi,
.task-queue-event.confirmed .msi {
  color: #16a34a;
}

.lh-footer {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 20px 16px;
  border-top: var(--cerne-border);
  flex-shrink: 0;
}

@media (max-width: 640px) {
  .lh {
    flex-direction: column;
    height: min(620px, 82vh);
  }

  .lh-nav {
    flex: 0 0 auto;
    flex-direction: row;
    flex-wrap: wrap;
    border-right: none;
    border-bottom: var(--cerne-border);
  }

  .lh-nav-label,
  .lh-nav-status {
    display: none;
  }

  .lh-nav-item {
    flex: 1;
    justify-content: center;
  }
}
</style>
