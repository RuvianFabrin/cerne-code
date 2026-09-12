<script setup lang="ts">
import { computed, onMounted, ref, watch } from "vue";
import { useI18n } from "vue-i18n";
import { open, save } from "@tauri-apps/plugin-dialog";
import { useSessionStore } from "../stores/session";
import Dialog from "primevue/dialog";
import Accordion from "primevue/accordion";
import AccordionPanel from "primevue/accordionpanel";
import AccordionHeader from "primevue/accordionheader";
import AccordionContent from "primevue/accordioncontent";
import {
  api,
  type CustomProviderConfig,
  type McpServerConfig,
  type McpToolInfo,
  type ProviderKind,
  type SearchProviderKind,
  type SearchConfigView,
} from "../api";
import { PROVIDER_KINDS, providerLabel, useProviderStore } from "../stores/provider";
import { SUPPORTED_LOCALES, setLocale, type LocaleCode } from "../i18n";
import { fontSettings, FONT_SIZE_LIMITS, setFontSetting, resetFontSettings } from "../fontSettings";
import LlamaForkRow from "./LlamaForkRow.vue";
import ModelBrowserDialog from "./ModelBrowserDialog.vue";
import { MCP_CONNECTORS, type McpConnector } from "../content/mcpConnectors";
import { executablePickerExtensions, llamaServerPlaceholder } from "../platform";

// Fase F do roteiro (14_backlog_pendente.md): Settings deixa de ser uma
// view inteira alternada em App.vue e vira modal, como Ajuda/Sobre já são
// — assim o usuário não perde o que estava digitando no composer ao abrir
// as configurações.
const props = defineProps<{ visible: boolean }>();
const emit = defineEmits<{ "update:visible": [value: boolean] }>();

const { t, locale } = useI18n();
const providerStore = useProviderStore();
const sessionStore = useSessionStore();
const openrouterKeyInput = ref("");
const editingOpenrouterKey = ref(false);

function startEditOpenrouterKey() {
  openrouterKeyInput.value = "";
  editingOpenrouterKey.value = true;
}

async function clearOpenrouterKey() {
  await providerStore.clearOpenrouterKey();
  editingOpenrouterKey.value = false;
}

function onLocaleChange(value: string) {
  setLocale(value as LocaleCode);
}

// Estado do modal de navegação de modelos — um só modal reutilizado pra
// qualquer provedor/conexão; `openModelBrowser` define quem ele mostra.
const modelBrowser = ref({
  visible: false,
  kind: "openrouter" as ProviderKind,
  forkId: "",
  customProviderId: "",
  title: "",
});

// Seleção de conexão pra voz (TTS/STT, seção "Voz" abaixo) — mesmas
// conexões já configuradas em qualquer outro lugar do app (OpenRouter,
// llama.cpp, Ollama, LM Studio, custom), a pedido do usuário
// ("permitir o usuario mudar em configurações, inclusive usando outros
// provedores pré configurados"). Default nasce em OpenRouter
// (`AppConfig::default`, backend) porque é a única conexão que já vem com
// endpoint de áudio testado.
const voiceProviderOptions = computed(() => PROVIDER_KINDS.map((kind) => ({ kind, label: providerLabel(kind) })));

function setTtsProvider(kind: ProviderKind) {
  if (!providerStore.config) return;
  providerStore.config.tts_provider = kind;
  if (kind === "llama_cpp" && !providerStore.config.tts_llama_fork) {
    providerStore.config.tts_llama_fork = providerStore.forks[0]?.id ?? null;
  }
  if (kind === "custom" && !providerStore.config.tts_custom_provider_id) {
    providerStore.config.tts_custom_provider_id = providerStore.customProviders[0]?.id ?? null;
  }
  providerStore.saveConfig();
}

function setSttProvider(kind: ProviderKind) {
  if (!providerStore.config) return;
  providerStore.config.stt_provider = kind;
  if (kind === "llama_cpp" && !providerStore.config.stt_llama_fork) {
    providerStore.config.stt_llama_fork = providerStore.forks[0]?.id ?? null;
  }
  if (kind === "custom" && !providerStore.config.stt_custom_provider_id) {
    providerStore.config.stt_custom_provider_id = providerStore.customProviders[0]?.id ?? null;
  }
  providerStore.saveConfig();
}

function openModelBrowser(kind: ProviderKind, title: string, forkId = "", customProviderId = "") {
  modelBrowser.value = { visible: true, kind, forkId, customProviderId, title };
}

const newForkId = ref("");
const newForkLabel = ref("");
const newForkExe = ref("");
const newForkIni = ref("");
const newForkPort = ref(8082);
const forkError = ref("");

async function pickForkExe() {
  // Filtro condicional por SO (Tarefa 4.1 do port): no Windows filtramos
  // .exe; fora dele executavel nao tem extensao — sem filtro nenhum.
  const extensions = executablePickerExtensions();
  const selected = await open({
    directory: false,
    multiple: false,
    ...(extensions ? { filters: [{ name: t("settings.executableFilter"), extensions }] } : {}),
  });
  if (typeof selected === "string") newForkExe.value = selected;
}

async function pickForkIni() {
  const selected = await open({ directory: false, multiple: false, filters: [{ name: t("settings.iniConfigFilter"), extensions: ["ini"] }] });
  if (typeof selected === "string") newForkIni.value = selected;
}

async function addFork() {
  if (!newForkId.value.trim() || !newForkExe.value.trim() || !newForkIni.value.trim()) return;
  forkError.value = "";
  try {
    await providerStore.addLlamaFork({
      id: newForkId.value.trim(),
      label: newForkLabel.value.trim() || newForkId.value.trim(),
      server_exe: newForkExe.value.trim(),
      models_ini: newForkIni.value.trim(),
      port: newForkPort.value,
    });
    newForkId.value = "";
    newForkLabel.value = "";
    newForkExe.value = "";
    newForkIni.value = "";
    newForkPort.value = 8082;
  } catch (e) {
    forkError.value = String(e);
  }
}

const newCustomId = ref("");
const newCustomLabel = ref("");
const newCustomBaseUrl = ref("");
const newCustomApiKey = ref("");
const newCustomSupportsVision = ref(false);
const newCustomContextLength = ref<number | null>(null);
const customError = ref("");
const editingCustomId = ref<string | null>(null);

type CustomTestStatus = "idle" | "testing" | "success" | "error";
const customTestStatus = ref<CustomTestStatus>("idle");
const customTestModels = ref<string[]>([]);
const customTestError = ref("");

function resetCustomForm() {
  editingCustomId.value = null;
  newCustomId.value = "";
  newCustomLabel.value = "";
  newCustomBaseUrl.value = "";
  newCustomApiKey.value = "";
  newCustomSupportsVision.value = false;
  newCustomContextLength.value = null;
  customTestStatus.value = "idle";
  customTestModels.value = [];
  customTestError.value = "";
}

async function startEditCustomProvider(provider: CustomProviderConfig) {
  editingCustomId.value = provider.id;
  newCustomId.value = provider.id;
  newCustomLabel.value = provider.label;
  newCustomBaseUrl.value = provider.base_url;
  newCustomApiKey.value = "";
  newCustomSupportsVision.value = provider.supports_vision;
  newCustomContextLength.value = provider.context_length;
  customTestStatus.value = "idle";
  customTestModels.value = [];
  customTestError.value = "";
}

async function testCustomProvider() {
  if (!newCustomBaseUrl.value.trim()) return;
  customTestStatus.value = "testing";
  customTestError.value = "";
  try {
    const models = await api.testCustomProvider(newCustomBaseUrl.value.trim(), newCustomApiKey.value.trim() || undefined);
    customTestModels.value = models.map((m) => m.id);
    customTestStatus.value = "success";
  } catch (e) {
    customTestError.value = String(e);
    customTestStatus.value = "error";
  }
}

async function saveCustomProvider() {
  const id = (editingCustomId.value ?? newCustomId.value).trim();
  if (!id || !newCustomBaseUrl.value.trim()) return;
  customError.value = "";
  try {
    await providerStore.addCustomProvider(
      {
        id,
        label: newCustomLabel.value.trim() || id,
        base_url: newCustomBaseUrl.value.trim(),
        supports_vision: newCustomSupportsVision.value,
        context_length: newCustomContextLength.value || null,
      },
      newCustomApiKey.value.trim() || undefined,
    );
    resetCustomForm();
  } catch (e) {
    customError.value = String(e);
  }
}

async function removeCustomProvider(id: string) {
  await providerStore.removeCustomProvider(id);
  if (editingCustomId.value === id) resetCustomForm();
}

const mcpServers = ref<McpServerConfig[]>([]);
const newMcpName = ref("");
const newMcpCommand = ref("");
const newMcpArgs = ref("");
const newMcpEnv = ref("");
// Servidor remoto (HTTP streamable, pedido do usuário 2026-08-18 — a maioria
// dos MCPs de produto SaaS hoje é hospedada, não um pacote npm local) usa
// url+token em vez de comando/args/env.
const newMcpIsRemote = ref(false);
const newMcpUrl = ref("");
const newMcpBearerToken = ref("");
const mcpError = ref("");
const mcpConnectorsExpanded = ref(false);
// null = criando um servidor novo; preenchido = editando um já existente (o
// campo nome vira somente-leitura, já que é a chave que identifica o
// servidor — trocar o nome aqui criaria um servidor novo em vez de editar).
const editingMcpName = ref<string | null>(null);

type McpTestStatus = "idle" | "testing" | "success" | "error";
const mcpTestStatus = ref<McpTestStatus>("idle");
const mcpTestTools = ref<McpToolInfo[]>([]);
const mcpTestError = ref("");

async function loadMcpServers() {
  mcpServers.value = await api.listMcpServers();
}

/** `KEY=VALOR` uma por linha — formato mais fácil de editar numa textarea do
 * que um editor de mapa chave/valor de verdade, e o que a maioria dos
 * READMEs de servidor MCP já mostra como exemplo de env. */
function parseEnvLines(text: string): Record<string, string> {
  const env: Record<string, string> = {};
  for (const line of text.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed || !trimmed.includes("=")) continue;
    const idx = trimmed.indexOf("=");
    env[trimmed.slice(0, idx).trim()] = trimmed.slice(idx + 1).trim();
  }
  return env;
}

function serializeEnvLines(env: Record<string, string>): string {
  return Object.entries(env)
    .map(([k, v]) => `${k}=${v}`)
    .join("\n");
}

function buildServerFromForm(): McpServerConfig | null {
  const name = (editingMcpName.value ?? newMcpName.value).trim();
  if (!name) return null;
  const existing = mcpServers.value.find((s) => s.name === name);
  if (newMcpIsRemote.value) {
    if (!newMcpUrl.value.trim()) return null;
    return {
      name,
      command: "",
      args: [],
      env: {},
      url: newMcpUrl.value.trim(),
      bearer_token: newMcpBearerToken.value.trim() || null,
      enabled: existing?.enabled ?? true,
    };
  }
  if (!newMcpCommand.value.trim()) return null;
  return {
    name,
    command: newMcpCommand.value.trim(),
    args: newMcpArgs.value.trim().length > 0 ? newMcpArgs.value.trim().split(/\s+/) : [],
    env: parseEnvLines(newMcpEnv.value),
    url: null,
    bearer_token: null,
    enabled: existing?.enabled ?? true,
  };
}

function resetMcpForm() {
  editingMcpName.value = null;
  newMcpName.value = "";
  newMcpCommand.value = "";
  newMcpArgs.value = "";
  newMcpEnv.value = "";
  newMcpIsRemote.value = false;
  newMcpUrl.value = "";
  newMcpBearerToken.value = "";
  mcpTestStatus.value = "idle";
  mcpTestTools.value = [];
  mcpTestError.value = "";
}

// Preenche o formulário de adicionar (NÃO salva sozinho) a partir de um
// conector pré-curado — usuário ainda revisa/completa chaves e clica
// "Adicionar" ele mesmo, mesmo espírito do preview obrigatório do import de
// skill por URL (nunca "instala direto").
function useMcpConnector(connector: McpConnector) {
  editingMcpName.value = null;
  newMcpName.value = connector.id;
  if (connector.url) {
    newMcpIsRemote.value = true;
    newMcpUrl.value = connector.url;
    newMcpBearerToken.value = "";
    newMcpCommand.value = "";
    newMcpArgs.value = "";
    newMcpEnv.value = "";
  } else {
    newMcpIsRemote.value = false;
    newMcpUrl.value = "";
    newMcpBearerToken.value = "";
    newMcpCommand.value = connector.command;
    newMcpArgs.value = connector.args.join(" ");
    newMcpEnv.value = connector.envKeys.map((k) => `${k}=`).join("\n");
  }
  mcpTestStatus.value = "idle";
  mcpTestTools.value = [];
  mcpTestError.value = "";
}

function startEditMcpServer(server: McpServerConfig) {
  editingMcpName.value = server.name;
  newMcpName.value = server.name;
  newMcpIsRemote.value = !!server.url;
  newMcpUrl.value = server.url ?? "";
  newMcpBearerToken.value = server.bearer_token ?? "";
  newMcpCommand.value = server.command;
  newMcpArgs.value = server.args.join(" ");
  newMcpEnv.value = serializeEnvLines(server.env);
  mcpTestStatus.value = "idle";
  mcpTestTools.value = [];
  mcpTestError.value = "";
}

/** Sobe o servidor numa conexão descartável e lista as tools ANTES de
 * salvar — mesma ideia do teste de conexão que o LM Studio faz, pra não
 * salvar uma config que na prática não conecta e só descobrir isso na hora
 * de usar. */
async function testMcpServer() {
  const server = buildServerFromForm();
  if (!server) return;
  mcpTestStatus.value = "testing";
  mcpTestError.value = "";
  try {
    mcpTestTools.value = await api.testMcpServer(server);
    mcpTestStatus.value = "success";
  } catch (e) {
    mcpTestError.value = String(e);
    mcpTestStatus.value = "error";
  }
}

async function saveMcpServer() {
  const server = buildServerFromForm();
  if (!server) return;
  mcpError.value = "";
  try {
    await api.addMcpServer(server);
    resetMcpForm();
    await loadMcpServers();
  } catch (e) {
    mcpError.value = String(e);
  }
}

async function removeMcpServer(name: string) {
  await api.removeMcpServer(name);
  if (editingMcpName.value === name) resetMcpForm();
  await loadMcpServers();
}

async function toggleMcpServer(server: McpServerConfig) {
  await api.addMcpServer({ ...server, enabled: !server.enabled });
  await loadMcpServers();
}

const SEARCH_PROVIDER_OPTIONS = computed<{ value: SearchProviderKind; label: string }[]>(() => [
  { value: "auto", label: t("settings.searchAuto") },
  { value: "brave", label: "Brave Search API" },
  { value: "tavily", label: "Tavily" },
  { value: "searxng", label: t("settings.searchSearxng") },
  { value: "serper", label: "Serper.dev" },
  { value: "exa", label: "Exa" },
  { value: "google_cse", label: "Google Custom Search" },
  { value: "bing", label: "Bing / Azure AI Search" },
]);

// Providers que so precisam de chave de API (mesmo campo/fluxo de Brave/Tavily).
const SIMPLE_KEY_PROVIDERS: SearchProviderKind[] = ["brave", "tavily", "serper", "exa"];

const searchProvider = ref<SearchProviderKind>("auto");
const searchSearxngUrl = ref("http://127.0.0.1:8888");
const searchGoogleCseId = ref("");
const searchBingEndpoint = ref("https://api.bing.microsoft.com/v7.0/search");
const searchApiKeyInput = ref("");
const searchHasBraveKey = ref(false);
const searchHasTavilyKey = ref(false);
const searchHasSerperKey = ref(false);
const searchHasExaKey = ref(false);
const searchHasGoogleKey = ref(false);
const searchHasBingKey = ref(false);
const searchError = ref("");
type SearchTestStatus = "idle" | "testing" | "success" | "error";
const searchTestStatus = ref<SearchTestStatus>("idle");
const searchTestCount = ref(0);
const searchTestError = ref("");

const searchHasKeyMap = computed<Partial<Record<SearchProviderKind, boolean>>>(() => ({
  brave: searchHasBraveKey.value,
  tavily: searchHasTavilyKey.value,
  serper: searchHasSerperKey.value,
  exa: searchHasExaKey.value,
  google_cse: searchHasGoogleKey.value,
  bing: searchHasBingKey.value,
}));
const searchProviderHasKey = computed(() => searchHasKeyMap.value[searchProvider.value] ?? false);
const searchProviderNeedsKey = computed(
  () => SIMPLE_KEY_PROVIDERS.includes(searchProvider.value) || searchProvider.value === "google_cse" || searchProvider.value === "bing",
);

function applySearchConfigView(cfg: SearchConfigView) {
  searchHasBraveKey.value = cfg.has_brave_key;
  searchHasTavilyKey.value = cfg.has_tavily_key;
  searchHasSerperKey.value = cfg.has_serper_key;
  searchHasExaKey.value = cfg.has_exa_key;
  searchHasGoogleKey.value = cfg.has_google_key;
  searchHasBingKey.value = cfg.has_bing_key;
}

async function loadSearchConfig() {
  const cfg = await api.getSearchConfig();
  searchProvider.value = cfg.provider;
  searchSearxngUrl.value = cfg.searxng_url;
  searchGoogleCseId.value = cfg.google_cse_id;
  searchBingEndpoint.value = cfg.bing_endpoint;
  applySearchConfigView(cfg);
  searchApiKeyInput.value = "";
  searchTestStatus.value = "idle";
}

async function testSearchProvider() {
  searchTestStatus.value = "testing";
  searchTestError.value = "";
  try {
    searchTestCount.value = await api.testSearchProvider(
      searchProvider.value,
      searchApiKeyInput.value.trim() || undefined,
      searchSearxngUrl.value.trim() || undefined,
      searchGoogleCseId.value.trim() || undefined,
      searchBingEndpoint.value.trim() || undefined,
    );
    searchTestStatus.value = "success";
  } catch (e) {
    searchTestError.value = String(e);
    searchTestStatus.value = "error";
  }
}

async function saveSearchConfig() {
  searchError.value = "";
  try {
    const cfg = await api.saveSearchConfig(
      searchProvider.value,
      searchSearxngUrl.value.trim(),
      searchApiKeyInput.value.trim() || undefined,
      searchGoogleCseId.value.trim() || undefined,
      searchBingEndpoint.value.trim() || undefined,
    );
    applySearchConfigView(cfg);
    searchApiKeyInput.value = "";
  } catch (e) {
    searchError.value = String(e);
  }
}

async function clearSearchApiKey() {
  const cfg = await api.clearSearchApiKey(searchProvider.value);
  applySearchConfigView(cfg);
}

onMounted(() => {
  loadMcpServers();
  loadSearchConfig();
  loadGitBackupStatus();
  loadMemoryContent();
});

// Achado testando ao vivo (2026-08-18): `onMounted` só roda a primeira vez
// que o modal é criado — reabrir Configurações depois disso NUNCA recarrega
// nada, então "Memória entre sessões" ficava mostrando o valor de quando o
// app abriu, mesmo depois do LLM gravar um fato novo via `remember` numa
// sessão de chat enquanto o modal estava fechado. Diferente de MCP/busca/
// backup git (que só mudam através deste próprio modal), a memória pode ser
// escrita de FORA — precisa recarregar toda vez que o modal reabre, não só
// na primeira montagem.
watch(
  () => props.visible,
  (v) => {
    if (v) loadMemoryContent();
  },
);

async function saveKey() {
  if (!openrouterKeyInput.value.trim()) return;
  await providerStore.saveOpenrouterKey(openrouterKeyInput.value.trim());
  openrouterKeyInput.value = "";
  editingOpenrouterKey.value = false;
}

// Backup de sessão como .zip (PLANOS/14_backlog_pendente.md, "backup/exportar
// sessão"). Comando faz a leitura/escrita no disco — frontend só escolhe o
// caminho (dialog nativo) e mostra o resultado.
const backupStatus = ref("");
const backupError = ref("");

async function exportCurrentSession() {
  const session = sessionStore.currentSession;
  if (!session) return;
  backupStatus.value = "";
  backupError.value = "";
  const destPath = await save({
    defaultPath: `${session.title.replace(/[\\/:*?"<>|]/g, "_")}.zip`,
    filters: [{ name: "Zip", extensions: ["zip"] }],
  });
  if (!destPath) return;
  try {
    const result = await api.exportSessionsBackup([session.id], destPath);
    backupStatus.value = t("settings.backupExportSuccess", { count: result.exported });
  } catch (e) {
    backupError.value = String(e);
  }
}

async function exportAllSessions() {
  backupStatus.value = "";
  backupError.value = "";
  const destPath = await save({
    defaultPath: "cerne-sessoes.zip",
    filters: [{ name: "Zip", extensions: ["zip"] }],
  });
  if (!destPath) return;
  try {
    const result = await api.exportSessionsBackup([], destPath);
    backupStatus.value = t("settings.backupExportSuccess", { count: result.exported });
  } catch (e) {
    backupError.value = String(e);
  }
}

// Backup/sincronização via git — alternativa ao .zip pra quem quer manter
// as sessões versionadas num remoto próprio (github/gitlab privado, etc.),
// sem precisar exportar/importar manualmente toda vez. Repo fica em
// <app_data_dir>/sessions (não o app_data_dir inteiro — ver comentário em
// git.rs sobre por quê). Cerne não gerencia credencial nenhuma: autenticação
// com o remoto é o git/SO do usuário (credential helper, SSH agent) que já
// resolve, igual um `git push` normal no terminal resolveria.
const gitBackupIsRepo = ref(false);
const gitBackupRemote = ref<string | null>(null);
const gitBackupRemoteInput = ref("");
const gitBackupSyncing = ref(false);
const gitBackupResult = ref("");
const gitBackupError = ref("");
// Identidade local (nome/email) + token — pedido do usuário (2026-08-18)
// depois de travar tentando sincronizar sem nenhum dos dois configurado: o
// git precisa de identidade pra commitar, e sem token o pull/push num
// remoto https fica esperando credencial que nunca vem (agora com timeout
// no backend, mas melhor nem chegar lá).
const gitBackupNameInput = ref("");
const gitBackupEmailInput = ref("");
const gitBackupHasIdentity = ref(false);
const gitBackupHasToken = ref(false);
const gitBackupTokenPreview = ref<string | null>(null);
const gitBackupTokenInput = ref("");
const gitBackupEditingToken = ref(false);

async function loadGitBackupStatus() {
  try {
    const status = await api.backupGitStatus();
    gitBackupIsRepo.value = status.is_repo;
    gitBackupRemote.value = status.remote;
    gitBackupRemoteInput.value = status.remote ?? "";
    gitBackupNameInput.value = status.identity_name ?? "";
    gitBackupEmailInput.value = status.identity_email ?? "";
    gitBackupHasIdentity.value = !!(status.identity_name && status.identity_email);
    gitBackupHasToken.value = status.has_token;
    gitBackupTokenPreview.value = status.token_preview;
  } catch {
    // silencioso — só some a seção de status, não impede o resto de Configurações.
  }
}

async function saveGitBackupIdentity() {
  if (!gitBackupNameInput.value.trim() || !gitBackupEmailInput.value.trim()) return;
  gitBackupError.value = "";
  try {
    await api.backupGitSetIdentity(gitBackupNameInput.value.trim(), gitBackupEmailInput.value.trim());
    await loadGitBackupStatus();
  } catch (e) {
    gitBackupError.value = String(e);
  }
}

async function saveGitBackupToken() {
  if (!gitBackupTokenInput.value.trim()) return;
  gitBackupError.value = "";
  try {
    await api.setBackupGitToken(gitBackupTokenInput.value.trim());
    gitBackupTokenInput.value = "";
    gitBackupEditingToken.value = false;
    await loadGitBackupStatus();
  } catch (e) {
    gitBackupError.value = String(e);
  }
}

async function clearGitBackupToken() {
  gitBackupError.value = "";
  try {
    await api.clearBackupGitToken();
    await loadGitBackupStatus();
  } catch (e) {
    gitBackupError.value = String(e);
  }
}

async function initGitBackup() {
  gitBackupError.value = "";
  try {
    await api.backupGitInit();
    await loadGitBackupStatus();
  } catch (e) {
    gitBackupError.value = String(e);
  }
}

async function saveGitBackupRemote() {
  if (!gitBackupRemoteInput.value.trim()) return;
  gitBackupError.value = "";
  try {
    await api.backupGitSetRemote(gitBackupRemoteInput.value.trim());
    await loadGitBackupStatus();
  } catch (e) {
    gitBackupError.value = String(e);
  }
}

async function syncGitBackup() {
  gitBackupSyncing.value = true;
  gitBackupResult.value = "";
  gitBackupError.value = "";
  try {
    gitBackupResult.value = await api.backupGitSync();
  } catch (e) {
    gitBackupError.value = String(e);
  } finally {
    gitBackupSyncing.value = false;
  }
}

// Memória entre sessões (inspirado no Hermes Agent) — o LLM grava fatos
// duráveis via a tool `remember` (só acrescenta), mas o usuário pode editar
// o arquivo inteiro livremente aqui (corrigir/apagar uma entrada).
const memoryContent = ref("");
const memorySaved = ref(false);

async function loadMemoryContent() {
  try {
    memoryContent.value = await api.getMemory();
  } catch {
    memoryContent.value = "";
  }
}

async function saveMemoryContent() {
  await api.setMemory(memoryContent.value);
  memorySaved.value = true;
  setTimeout(() => (memorySaved.value = false), 2000);
}

async function importSessionsBackup() {
  backupStatus.value = "";
  backupError.value = "";
  const sourcePath = await open({
    multiple: false,
    filters: [{ name: "Zip", extensions: ["zip"] }],
  });
  if (typeof sourcePath !== "string") return;
  try {
    const result = await api.importSessionsBackup(sourcePath);
    backupStatus.value = t("settings.backupImportSuccess", {
      imported: result.imported.length,
      skipped: result.skipped_invalid.length,
    });
    await sessionStore.loadSessions();
  } catch (e) {
    backupError.value = String(e);
  }
}

</script>

<template>
  <Dialog
    :visible="visible"
    @update:visible="(v) => emit('update:visible', v)"
    modal
    maximizable
    :header="$t('settings.title')"
    :style="{ width: '860px' }"
    class="settings-dialog"
  >
  <div class="settings">
    <div class="settings-inner">
      <Accordion multiple>
      <AccordionPanel value="language">
        <AccordionHeader>{{ $t("settings.language") }}</AccordionHeader>
        <AccordionContent>
        <p class="hint">{{ $t("settings.languageHint") }}</p>
        <select :value="locale" class="text-input" @change="onLocaleChange(($event.target as HTMLSelectElement).value)">
          <option v-for="l in SUPPORTED_LOCALES" :key="l.code" :value="l.code">{{ l.label }}</option>
        </select>
        </AccordionContent>
      </AccordionPanel>

      <AccordionPanel value="appearance">
        <AccordionHeader>{{ $t("settings.appearance") }}</AccordionHeader>
        <AccordionContent>
        <p class="hint">{{ $t("settings.appearanceHint") }}</p>

        <div class="font-setting-row">
          <label class="font-setting-label">
            {{ $t("settings.fontChat") }}
            <span class="font-setting-value">{{ fontSettings.chat }}px</span>
          </label>
          <input
            type="range"
            class="font-slider"
            :min="FONT_SIZE_LIMITS.chat.min"
            :max="FONT_SIZE_LIMITS.chat.max"
            :step="FONT_SIZE_LIMITS.chat.step"
            :value="fontSettings.chat"
            @input="setFontSetting('chat', Number(($event.target as HTMLInputElement).value))"
          />
          <p class="font-setting-preview" :style="{ fontSize: fontSettings.chat + 'px' }">
            {{ $t("settings.fontPreviewText") }}
          </p>
        </div>

        <div class="font-setting-row">
          <label class="font-setting-label">
            {{ $t("settings.fontComposer") }}
            <span class="font-setting-value">{{ fontSettings.composer }}px</span>
          </label>
          <input
            type="range"
            class="font-slider"
            :min="FONT_SIZE_LIMITS.composer.min"
            :max="FONT_SIZE_LIMITS.composer.max"
            :step="FONT_SIZE_LIMITS.composer.step"
            :value="fontSettings.composer"
            @input="setFontSetting('composer', Number(($event.target as HTMLInputElement).value))"
          />
        </div>

        <div class="font-setting-row">
          <label class="font-setting-label">
            {{ $t("settings.fontZoom") }}
            <span class="font-setting-value">{{ Math.round(fontSettings.zoom * 100) }}%</span>
          </label>
          <input
            type="range"
            class="font-slider"
            :min="FONT_SIZE_LIMITS.zoom.min"
            :max="FONT_SIZE_LIMITS.zoom.max"
            :step="FONT_SIZE_LIMITS.zoom.step"
            :value="fontSettings.zoom"
            @input="setFontSetting('zoom', Number(($event.target as HTMLInputElement).value))"
          />
          <p class="hint" style="margin: 4px 0 0">{{ $t("settings.fontZoomHint") }}</p>
        </div>

        <button class="btn-secondary" @click="resetFontSettings">{{ $t("settings.fontReset") }}</button>
        </AccordionContent>
      </AccordionPanel>

      <AccordionPanel value="openrouter">
        <AccordionHeader>OpenRouter</AccordionHeader>
        <AccordionContent>
        <p class="hint">{{ $t("settings.apiKeyVaultHint") }}</p>
        <div v-if="providerStore.hasOpenrouterKey && !editingOpenrouterKey" class="key-status-row">
          <span class="key-status-chip">
            <span class="msi">check_circle</span>
            <span class="key-preview">{{ providerStore.openrouterKeyPreview }}</span>
          </span>
          <button class="btn-secondary" @click="startEditOpenrouterKey">{{ $t("settings.changeKey") }}</button>
          <button class="btn-secondary" @click="clearOpenrouterKey">{{ $t("settings.removeKey") }}</button>
        </div>
        <div v-else class="key-row">
          <input v-model="openrouterKeyInput" type="password" placeholder="sk-or-..." class="text-input" />
          <button class="btn-primary" @click="saveKey">{{ $t("sidebar.save") }}</button>
          <button v-if="providerStore.hasOpenrouterKey" class="btn-secondary" @click="editingOpenrouterKey = false">
            {{ $t("newSession.cancel") }}
          </button>
        </div>
        <button class="btn-secondary browse-models-btn" @click="openModelBrowser('openrouter', $t('settings.modelsOpenRouterTitle'))">
          <span class="msi">search</span>
          {{ $t("settings.viewModels") }}
        </button>
        </AccordionContent>
      </AccordionPanel>

      <AccordionPanel value="llama-cpp-local">
        <AccordionHeader>{{ $t("settings.llamaCppLocal") }}</AccordionHeader>
        <AccordionContent>
        <p class="hint" v-html="$t('settings.llamaCppHint')"></p>
        <div class="fork-list">
          <LlamaForkRow
            v-for="fork in providerStore.forks"
            :key="fork.id"
            :fork="fork"
            @browse="openModelBrowser('llama_cpp', `Modelos — ${fork.label}`, fork.id)"
          />
          <p v-if="providerStore.forks.length === 0" class="hint">{{ $t("settings.noForksYet") }}</p>
        </div>
        <div class="fork-form">
          <div class="fork-form-row">
            <input v-model="newForkId" class="text-input" placeholder="id (ex: turboquant)" />
            <input v-model="newForkLabel" class="text-input" :placeholder="$t('settings.labelOptional')" />
            <input v-model.number="newForkPort" type="number" class="text-input fork-port-input" :placeholder="$t('settings.port')" />
          </div>
          <div class="fork-form-row">
            <button class="folder-btn" @click="pickForkExe">
              <span class="msi">folder_open</span>
              <span class="folder-path">{{ newForkExe || llamaServerPlaceholder() }}</span>
            </button>
            <button class="folder-btn" @click="pickForkIni">
              <span class="msi">folder_open</span>
              <span class="folder-path">{{ newForkIni || "models.ini..." }}</span>
            </button>
          </div>
          <button class="btn-primary" @click="addFork">{{ $t("settings.addFork") }}</button>
        </div>
        <p v-if="forkError" class="error-text">{{ forkError }}</p>
        </AccordionContent>
      </AccordionPanel>

      <AccordionPanel value="custom-providers">
        <AccordionHeader>{{ $t("settings.customProviders") }}</AccordionHeader>
        <AccordionContent>
        <p class="hint" v-html="$t('settings.customProvidersHint')"></p>
        <div class="skill-list">
          <div v-for="provider in providerStore.customProviders" :key="provider.id" class="skill-row mcp-row">
            <div class="skill-info">
              <span class="skill-name">
                {{ provider.label }}
                <span v-if="provider.supports_vision" class="vision-badge" v-tooltip.top="$t('settings.visionEnabledConnection')">
                  <span class="msi">image</span>
                </span>
              </span>
              <span class="skill-desc">{{ provider.base_url }}</span>
            </div>
            <div class="mcp-actions">
              <button class="btn-secondary" @click="openModelBrowser('custom', $t('settings.modelsForTitle', { name: provider.label }), '', provider.id)">{{ $t("settings.viewModels") }}</button>
              <button class="btn-secondary" @click="startEditCustomProvider(provider)">{{ $t("settings.edit") }}</button>
              <button class="btn-secondary" @click="removeCustomProvider(provider.id)">{{ $t("settings.remove") }}</button>
            </div>
          </div>
          <p v-if="providerStore.customProviders.length === 0" class="hint">{{ $t("settings.noCustomProvidersYet") }}</p>
        </div>
        <div class="mcp-form">
          <div class="mcp-form-row">
            <input
              v-model="newCustomId"
              class="text-input"
              placeholder="id (ex: claude)"
              :disabled="!!editingCustomId"
              v-tooltip.top="editingCustomId ? $t('settings.idCannotChange') : ''"
            />
            <input v-model="newCustomLabel" class="text-input" placeholder="rótulo (ex: Claude)" />
          </div>
          <input v-model="newCustomBaseUrl" class="text-input" placeholder="URL base (ex: https://api.anthropic.com/v1/)" />
          <input
            v-model="newCustomApiKey"
            type="password"
            class="text-input"
            :placeholder="editingCustomId ? $t('settings.newApiKeyPlaceholder') : $t('settings.apiKeyPlaceholder')"
          />
          <label class="vision-checkbox">
            <input type="checkbox" v-model="newCustomSupportsVision" />
            {{ $t("settings.visionCheckboxLabel") }}
          </label>
          <input
            v-model.number="newCustomContextLength"
            type="number"
            class="text-input"
            :placeholder="$t('settings.contextLengthPlaceholder')"
          />
          <div class="mcp-form-actions">
            <button class="btn-secondary" :disabled="customTestStatus === 'testing'" @click="testCustomProvider">
              {{ customTestStatus === "testing" ? $t("settings.testing") : $t("settings.testConnection") }}
            </button>
            <button class="btn-primary" @click="saveCustomProvider">{{ editingCustomId ? $t("sidebar.save") : $t("settings.add") }}</button>
            <button v-if="editingCustomId" class="btn-secondary" @click="resetCustomForm">{{ $t("newSession.cancel") }}</button>
          </div>
          <p v-if="customTestStatus === 'success'" class="mcp-test-success">
            <span class="msi">check_circle</span>
            {{ $t("settings.connectedModelsFound", { count: customTestModels.length, list: customTestModels.length ? ": " + customTestModels.slice(0, 8).join(", ") + (customTestModels.length > 8 ? "..." : "") : "" }) }}
          </p>
          <p v-if="customTestStatus === 'error'" class="error-text">{{ customTestError }}</p>
        </div>
        <p v-if="customError" class="error-text">{{ customError }}</p>
        </AccordionContent>
      </AccordionPanel>

      <AccordionPanel value="mcp-servers">
        <AccordionHeader>{{ $t("settings.mcpServers") }}</AccordionHeader>
        <AccordionContent>
        <p class="hint">
          {{ $t("settings.mcpHintBefore") }}
          <code>mcp__{{ '{servidor}' }}__{{ '{tool}' }}</code>{{ $t("settings.mcpHintAfter") }}
          <code>mcp_servers.json</code>{{ $t("settings.mcpHintAfter2") }}
          <code>mcpServers</code>{{ $t("settings.mcpHintAfter3") }}
        </p>
        <div class="skill-list">
          <div v-for="server in mcpServers" :key="server.name" class="skill-row mcp-row">
            <div class="skill-info">
              <span class="skill-name">
                {{ server.name }}
                <span v-if="server.url" class="asp-scope-badge">{{ $t("settings.mcpRemoteBadge") }}</span>
              </span>
              <span class="skill-desc">{{ server.url ? server.url : `${server.command} ${server.args.join(" ")}` }}</span>
            </div>
            <div class="mcp-actions">
              <button class="btn-secondary" @click="startEditMcpServer(server)">{{ $t("settings.edit") }}</button>
              <button class="btn-secondary" @click="toggleMcpServer(server)">
                {{ server.enabled ? $t("settings.disable") : $t("settings.enable") }}
              </button>
              <button class="btn-secondary" @click="removeMcpServer(server.name)">{{ $t("settings.remove") }}</button>
            </div>
          </div>
          <p v-if="mcpServers.length === 0" class="hint">{{ $t("settings.noMcpServersYet") }}</p>
        </div>
        <button class="btn-secondary asp-new" @click="mcpConnectorsExpanded = !mcpConnectorsExpanded">
          {{ mcpConnectorsExpanded ? $t("settings.hideMcpConnectors") : $t("settings.showMcpConnectors") }}
        </button>
        <div v-if="mcpConnectorsExpanded" class="skill-list mcp-connectors-list">
          <div v-for="connector in MCP_CONNECTORS" :key="connector.id" class="skill-row mcp-row">
            <span v-if="connector.icon.kind === 'svg'" class="mcp-connector-icon" :style="{ color: connector.icon.color }">
              <svg :viewBox="connector.icon.viewBox" xmlns="http://www.w3.org/2000/svg"><path :d="connector.icon.path" fill="currentColor" /></svg>
            </span>
            <span v-else class="mcp-connector-icon mcp-connector-icon-material msi">{{ connector.icon.name }}</span>
            <div class="skill-info">
              <span class="skill-name">{{ connector.name }}</span>
              <span class="skill-desc">{{ connector.description }}</span>
            </div>
            <div class="mcp-actions">
              <a :href="connector.docsUrl" target="_blank" rel="noopener" class="btn-secondary mcp-docs-link">{{ $t("settings.mcpConnectorDocs") }}</a>
              <button class="btn-secondary" @click="useMcpConnector(connector)">{{ $t("settings.mcpConnectorUse") }}</button>
            </div>
          </div>
        </div>
        <div class="mcp-form">
          <div class="mcp-form-row">
            <input
              v-model="newMcpName"
              class="text-input"
              placeholder="nome (ex: github)"
              :disabled="!!editingMcpName"
              v-tooltip.top="editingMcpName ? $t('settings.nameCannotChange') : ''"
            />
            <label class="mcp-remote-toggle">
              <input type="checkbox" v-model="newMcpIsRemote" />
              {{ $t("settings.mcpRemoteToggle") }}
            </label>
          </div>
          <template v-if="newMcpIsRemote">
            <input v-model="newMcpUrl" class="text-input" placeholder="https://mcp.exemplo.com/mcp" />
            <input v-model="newMcpBearerToken" type="password" class="text-input" :placeholder="$t('settings.mcpBearerTokenPlaceholder')" />
          </template>
          <template v-else>
            <div class="mcp-form-row">
              <input v-model="newMcpCommand" class="text-input" placeholder="comando (ex: npx)" />
            </div>
            <input v-model="newMcpArgs" class="text-input" placeholder="argumentos (ex: -y @escopo/pacote)" />
            <textarea
              v-model="newMcpEnv"
              class="text-input mcp-env-input"
              rows="2"
              :placeholder="$t('settings.envVarsPlaceholder')"
            />
          </template>
          <div class="mcp-form-actions">
            <button class="btn-secondary" :disabled="mcpTestStatus === 'testing'" @click="testMcpServer">
              {{ mcpTestStatus === "testing" ? $t("settings.testing") : $t("settings.testConnection") }}
            </button>
            <button class="btn-primary" @click="saveMcpServer">{{ editingMcpName ? $t("sidebar.save") : $t("settings.add") }}</button>
            <button v-if="editingMcpName" class="btn-secondary" @click="resetMcpForm">{{ $t("newSession.cancel") }}</button>
          </div>
          <p v-if="mcpTestStatus === 'success'" class="mcp-test-success">
            <span class="msi">check_circle</span>
            {{ $t("settings.connectedToolsFound", { count: mcpTestTools.length, list: mcpTestTools.length ? ": " + mcpTestTools.map((t) => t.name).join(", ") : "" }) }}
          </p>
          <p v-if="mcpTestStatus === 'error'" class="error-text">{{ mcpTestError }}</p>
        </div>
        <p v-if="mcpError" class="error-text">{{ mcpError }}</p>
        </AccordionContent>
      </AccordionPanel>

      <AccordionPanel value="memory">
        <AccordionHeader>{{ $t("settings.memoryTitle") }}</AccordionHeader>
        <AccordionContent>
        <p class="hint">{{ $t("settings.memoryHint") }}</p>
        <textarea
          v-model="memoryContent"
          class="text-input memory-textarea"
          rows="6"
          :placeholder="$t('settings.memoryPlaceholder')"
        />
        <div class="mcp-form-actions">
          <button class="btn-primary" @click="saveMemoryContent">{{ $t("sidebar.save") }}</button>
          <span v-if="memorySaved" class="mcp-test-success">
            <span class="msi">check_circle</span>
            {{ $t("settings.memorySaved") }}
          </span>
        </div>
        </AccordionContent>
      </AccordionPanel>

      <!-- Escondido a pedido do usuário (2026-08-20), antes de subir o Cerne
           Code — backup .zip e backup via git ainda não foram confirmados
           testando (ver PLANOS/Testar.md, itens 1 e 2). Código intacto,
           só a UI fica invisível até serem confirmados; então é só tirar
           esse `v-if="false"`. -->
      <AccordionPanel v-if="false" value="backup">
        <AccordionHeader>{{ $t("settings.backupTitle") }}</AccordionHeader>
        <AccordionContent>
        <p class="hint">{{ $t("settings.backupHint") }}</p>
        <div class="mcp-form-actions">
          <button v-if="sessionStore.currentSession" class="btn-secondary" @click="exportCurrentSession">
            {{ $t("settings.backupExportCurrent") }}
          </button>
          <button class="btn-secondary" @click="exportAllSessions">{{ $t("settings.backupExportAll") }}</button>
          <button class="btn-primary" @click="importSessionsBackup">{{ $t("settings.backupImport") }}</button>
        </div>
        <p v-if="backupStatus" class="mcp-test-success">
          <span class="msi">check_circle</span>
          {{ backupStatus }}
        </p>
        <p v-if="backupError" class="error-text">{{ backupError }}</p>

        <div class="git-backup-block">
          <p class="hint">{{ $t("settings.gitBackupHint") }}</p>
          <button v-if="!gitBackupIsRepo" class="btn-secondary" @click="initGitBackup">
            {{ $t("settings.gitBackupInit") }}
          </button>
          <template v-else>
            <div class="mcp-form-row">
              <input
                v-model="gitBackupRemoteInput"
                class="text-input"
                :placeholder="$t('settings.gitBackupRemotePlaceholder')"
              />
              <button class="btn-secondary" @click="saveGitBackupRemote">{{ $t("settings.gitBackupSetRemote") }}</button>
            </div>
            <p v-if="gitBackupRemote" class="hint">{{ $t("settings.gitBackupCurrentRemote", { url: gitBackupRemote }) }}</p>

            <p class="hint">{{ $t("settings.gitBackupIdentityHint") }}</p>
            <div class="mcp-form-row">
              <input
                v-model="gitBackupNameInput"
                class="text-input"
                :placeholder="$t('settings.gitBackupNamePlaceholder')"
              />
              <input
                v-model="gitBackupEmailInput"
                class="text-input"
                :placeholder="$t('settings.gitBackupEmailPlaceholder')"
              />
              <button class="btn-secondary" @click="saveGitBackupIdentity">{{ $t("sidebar.save") }}</button>
            </div>
            <p v-if="gitBackupHasIdentity" class="hint">
              {{ $t("settings.gitBackupCurrentIdentity", { name: gitBackupNameInput, email: gitBackupEmailInput }) }}
            </p>

            <p class="hint">{{ $t("settings.gitBackupTokenHint") }}</p>
            <div v-if="gitBackupHasToken && !gitBackupEditingToken" class="key-status-row">
              <span class="key-status-chip">
                <span class="msi">check_circle</span>
                <span class="key-preview">{{ gitBackupTokenPreview }}</span>
              </span>
              <button class="btn-secondary" @click="gitBackupEditingToken = true">{{ $t("settings.changeKey") }}</button>
              <button class="btn-secondary" @click="clearGitBackupToken">{{ $t("settings.removeKey") }}</button>
            </div>
            <div v-else class="key-row">
              <input v-model="gitBackupTokenInput" type="password" :placeholder="$t('settings.gitBackupTokenPlaceholder')" class="text-input" />
              <button class="btn-primary" @click="saveGitBackupToken">{{ $t("sidebar.save") }}</button>
            </div>

            <button class="btn-primary" :disabled="gitBackupSyncing" @click="syncGitBackup">
              {{ gitBackupSyncing ? $t("settings.gitBackupSyncing") : $t("settings.gitBackupSync") }}
            </button>
          </template>
          <p v-if="gitBackupResult" class="mcp-test-success">
            <span class="msi">check_circle</span>
            {{ gitBackupResult }}
          </p>
          <p v-if="gitBackupError" class="error-text">{{ gitBackupError }}</p>
        </div>
        </AccordionContent>
      </AccordionPanel>

      <AccordionPanel value="web-search">
        <AccordionHeader>{{ $t("settings.webSearch") }}</AccordionHeader>
        <AccordionContent>
        <p class="hint" v-html="$t('settings.webSearchHint')"></p>
        <div class="field">
          <label>Provider</label>
          <select v-model="searchProvider" class="text-input">
            <option v-for="opt in SEARCH_PROVIDER_OPTIONS" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
          </select>
        </div>
        <div v-if="searchProvider === 'google_cse'" class="field">
          <label>{{ $t("settings.googleCseId") }}</label>
          <input v-model="searchGoogleCseId" class="text-input" placeholder="ex: 017576662512468239146:omuauf_lfve" />
        </div>
        <div v-if="searchProvider === 'bing'" class="field">
          <label>{{ $t("settings.bingEndpoint") }}</label>
          <input v-model="searchBingEndpoint" class="text-input" placeholder="https://api.bing.microsoft.com/v7.0/search" />
        </div>
        <div v-if="searchProviderNeedsKey" class="field">
          <label>{{ $t("settings.apiKey") }}</label>
          <input
            v-model="searchApiKeyInput"
            type="password"
            class="text-input"
            :placeholder="searchProviderHasKey ? $t('settings.keyAlreadyConfigured') : $t('settings.apiKeyPlaceholder')"
          />
          <button v-if="searchProviderHasKey" class="btn-secondary" @click="clearSearchApiKey">
            {{ $t("settings.removeKey") }}
          </button>
        </div>
        <div v-if="searchProvider === 'searxng'" class="field">
          <label>{{ $t("settings.searxngUrl") }}</label>
          <input v-model="searchSearxngUrl" class="text-input" placeholder="http://127.0.0.1:8888" />
        </div>
        <div class="mcp-form-actions">
          <button class="btn-secondary" :disabled="searchTestStatus === 'testing'" @click="testSearchProvider">
            {{ searchTestStatus === "testing" ? $t("settings.testing") : $t("settings.testConnection") }}
          </button>
          <button class="btn-primary" @click="saveSearchConfig">{{ $t("sidebar.save") }}</button>
        </div>
        <p v-if="searchTestStatus === 'success'" class="mcp-test-success">
          <span class="msi">check_circle</span>
          {{ $t("settings.connectedResultsFound", { count: searchTestCount }) }}
        </p>
        <p v-if="searchTestStatus === 'error'" class="error-text">{{ searchTestError }}</p>
        <p v-if="searchError" class="error-text">{{ searchError }}</p>
        </AccordionContent>
      </AccordionPanel>

      <AccordionPanel v-if="providerStore.config" value="local-endpoints">
        <AccordionHeader>{{ $t("settings.localEndpoints") }}</AccordionHeader>
        <AccordionContent>
        <div class="field">
          <label>Ollama</label>
          <div class="endpoint-row">
            <input v-model="providerStore.config.ollama_base_url" class="text-input" @change="providerStore.saveConfig" />
            <button class="btn-secondary" @click="openModelBrowser('ollama', $t('settings.modelsForTitle', { name: 'Ollama' }))">{{ $t("settings.viewModels") }}</button>
          </div>
        </div>
        <div class="field">
          <label>LM Studio</label>
          <div class="endpoint-row">
            <input v-model="providerStore.config.lmstudio_base_url" class="text-input" @change="providerStore.saveConfig" />
            <button class="btn-secondary" @click="openModelBrowser('lm_studio', $t('settings.modelsForTitle', { name: 'LM Studio' }))">{{ $t("settings.viewModels") }}</button>
          </div>
        </div>
        <div class="field">
          <label>{{ $t("settings.llamaCppRouter") }}</label>
          <input v-model="providerStore.config.llama_cpp_base_url" class="text-input" @change="providerStore.saveConfig" />
        </div>
        </AccordionContent>
      </AccordionPanel>

      <AccordionPanel v-if="providerStore.config" value="voice">
        <AccordionHeader>{{ $t("settings.voiceTitle") }}</AccordionHeader>
        <AccordionContent>
        <p class="hint">{{ $t("settings.voiceHint") }}</p>

        <h3 class="subhead">{{ $t("settings.voiceTts") }}</h3>
        <div class="field">
          <label>{{ $t("settings.voiceBackend") }}</label>
          <select v-model="providerStore.config.tts_backend" @change="providerStore.saveConfig" class="text-input">
            <option value="openai_compatible">{{ $t("settings.voiceBackendOpenai") }}</option>
            <option value="voicebox">{{ $t("settings.voiceBackendVoicebox") }}</option>
          </select>
        </div>

        <template v-if="providerStore.config.tts_backend === 'voicebox'">
          <div class="field">
            <label>{{ $t("settings.voiceboxUrl") }}</label>
            <input v-model="providerStore.config.voicebox_base_url" class="text-input" @change="providerStore.saveConfig" />
          </div>
          <div class="field">
            <label>{{ $t("settings.voiceboxProfile") }}</label>
            <input v-model="providerStore.config.voicebox_tts_profile" class="text-input" @change="providerStore.saveConfig" :placeholder="$t('settings.voiceboxProfilePlaceholder')" />
          </div>
        </template>
        <template v-else>
          <div class="field">
            <label>{{ $t("providerPicker.connection") }}</label>
            <select :value="providerStore.config.tts_provider" @change="setTtsProvider(($event.target as HTMLSelectElement).value as ProviderKind)" class="text-input">
              <option v-for="opt in voiceProviderOptions" :key="opt.kind" :value="opt.kind">{{ opt.label }}</option>
            </select>
          </div>
          <div v-if="providerStore.config.tts_provider === 'llama_cpp'" class="field">
            <label>Fork</label>
            <select v-model="providerStore.config.tts_llama_fork" @change="providerStore.saveConfig" class="text-input">
              <option v-for="f in providerStore.forks" :key="f.id" :value="f.id">{{ f.label }}</option>
            </select>
          </div>
          <div v-if="providerStore.config.tts_provider === 'custom'" class="field">
            <label>{{ $t("providerPicker.connection") }}</label>
            <select v-model="providerStore.config.tts_custom_provider_id" @change="providerStore.saveConfig" class="text-input">
              <option v-for="p in providerStore.customProviders" :key="p.id" :value="p.id">{{ p.label }}</option>
            </select>
          </div>
          <div class="field">
            <label>{{ $t("settings.voiceModel") }}</label>
            <input v-model="providerStore.config.tts_model" class="text-input" @change="providerStore.saveConfig" />
          </div>
          <div class="field">
            <label>{{ $t("settings.voiceVoice") }}</label>
            <input v-model="providerStore.config.tts_voice" class="text-input" @change="providerStore.saveConfig" :placeholder="$t('settings.voiceVoicePlaceholder')" />
          </div>
        </template>
        <label class="checkbox-row">
          <input type="checkbox" v-model="providerStore.config.tts_auto_language" @change="providerStore.saveConfig" />
          {{ $t("settings.voiceAutoLanguage") }}
        </label>

        <h3 class="subhead">{{ $t("settings.voiceStt") }}</h3>
        <div class="field">
          <label>{{ $t("settings.voiceBackend") }}</label>
          <select v-model="providerStore.config.stt_backend" @change="providerStore.saveConfig" class="text-input">
            <option value="openai_compatible">{{ $t("settings.voiceBackendOpenai") }}</option>
            <option value="voicebox">{{ $t("settings.voiceBackendVoicebox") }}</option>
          </select>
        </div>

        <template v-if="providerStore.config.stt_backend === 'voicebox'">
          <div class="field">
            <label>{{ $t("settings.voiceboxUrl") }}</label>
            <input v-model="providerStore.config.voicebox_base_url" class="text-input" @change="providerStore.saveConfig" />
          </div>
          <div class="field">
            <label>{{ $t("settings.voiceboxSttLanguage") }}</label>
            <input v-model="providerStore.config.voicebox_stt_language" class="text-input" @change="providerStore.saveConfig" :placeholder="$t('settings.voiceboxSttLanguagePlaceholder')" />
          </div>
        </template>
        <template v-else>
          <div class="field">
            <label>{{ $t("providerPicker.connection") }}</label>
            <select :value="providerStore.config.stt_provider" @change="setSttProvider(($event.target as HTMLSelectElement).value as ProviderKind)" class="text-input">
              <option v-for="opt in voiceProviderOptions" :key="opt.kind" :value="opt.kind">{{ opt.label }}</option>
            </select>
          </div>
          <div v-if="providerStore.config.stt_provider === 'llama_cpp'" class="field">
            <label>Fork</label>
            <select v-model="providerStore.config.stt_llama_fork" @change="providerStore.saveConfig" class="text-input">
              <option v-for="f in providerStore.forks" :key="f.id" :value="f.id">{{ f.label }}</option>
            </select>
          </div>
          <div v-if="providerStore.config.stt_provider === 'custom'" class="field">
            <label>{{ $t("providerPicker.connection") }}</label>
            <select v-model="providerStore.config.stt_custom_provider_id" @change="providerStore.saveConfig" class="text-input">
              <option v-for="p in providerStore.customProviders" :key="p.id" :value="p.id">{{ p.label }}</option>
            </select>
          </div>
          <div class="field">
            <label>{{ $t("settings.voiceModel") }}</label>
            <input v-model="providerStore.config.stt_model" class="text-input" @change="providerStore.saveConfig" />
          </div>
        </template>
        </AccordionContent>
      </AccordionPanel>
      </Accordion>
    </div>

    <ModelBrowserDialog
      v-model:visible="modelBrowser.visible"
      :kind="modelBrowser.kind"
      :fork-id="modelBrowser.forkId"
      :custom-provider-id="modelBrowser.customProviderId"
      :title="modelBrowser.title"
    />
  </div>
  </Dialog>
</template>

<style scoped>
.settings {
  flex: 1;
  overflow-y: auto;
}

.settings-inner {
  max-width: 640px;
  margin: 0 auto;
  padding: 32px 24px 60px;
}

.browse-models-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  margin-top: 10px;
}

.browse-models-btn .msi {
  font-size: 16px;
}

.endpoint-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.endpoint-row .text-input {
  flex: 1;
  min-width: 0;
}

h1 {
  font-size: 20px;
  font-weight: 700;
  margin: 0 0 24px;
}

/* Cada seção de Configurações (OpenRouter, llama.cpp local, ...) virou um
   painel de acordeão, todos fechados por padrão — pedido do usuário
   (2026-08-20): "traz tudo fechado para dar uma organizada nessa design",
   a tela tinha crescido demais (11 seções sempre abertas, scroll gigante). */
:deep(.p-accordionpanel) {
  border-bottom: var(--cerne-border);
}

:deep(.p-accordionpanel:last-child) {
  border-bottom: none;
}

:deep(.p-accordionheader) {
  font-size: 14px;
  font-weight: 700;
  padding: 14px 4px;
  background: transparent;
  border: none;
  color: #18181b;
}

:deep(.p-accordioncontent-content) {
  padding: 0 4px 24px;
}

.hint {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
  margin: 0 0 12px;
}

.subhead {
  font-size: 13px;
  font-weight: 600;
  color: #3f3f46;
  margin: 16px 0 8px;
}

.font-setting-row {
  margin: 0 0 18px;
}

.font-setting-label {
  display: flex;
  align-items: center;
  justify-content: space-between;
  font-size: 13px;
  font-weight: 600;
  color: #3f3f46;
  margin-bottom: 6px;
}

.font-setting-value {
  font-family: var(--cerne-mono);
  font-weight: 500;
  color: #71717a;
}

.font-slider {
  width: 100%;
  accent-color: #18181b;
  cursor: pointer;
}

.font-setting-preview {
  margin: 8px 0 0;
  padding: 8px 10px;
  border: var(--cerne-border);
  border-radius: 8px;
  color: #18181b;
  background: #fafafa;
}

.key-row {
  display: flex;
  gap: 8px;
}

.key-status-row {
  display: flex;
  align-items: center;
  gap: 8px;
}

.key-status-chip {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 12px;
  border-radius: 8px;
  background: #f0fdf4;
  color: #15803d;
  font-size: 13px;
  font-weight: 600;
}

.key-status-chip .msi {
  font-size: 16px;
}

.key-preview {
  font-family: var(--cerne-mono);
  font-weight: 500;
}

.text-input {
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  outline: none;
  flex: 1;
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

.btn-secondary {
  border: var(--cerne-border);
  background: #ffffff;
  color: #52525b;
  border-radius: 8px;
  padding: 6px 10px;
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
}

.memory-textarea {
  width: 100%;
  box-sizing: border-box;
  resize: vertical;
  margin-bottom: 8px;
  font-family: inherit;
}

.git-backup-block {
  margin-top: 12px;
  padding-top: 12px;
  border-top: var(--cerne-border);
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.git-backup-block > button {
  align-self: flex-start;
}

.mcp-docs-link {
  text-decoration: none;
  display: inline-flex;
  align-items: center;
}

.mcp-connectors-list {
  margin-bottom: 12px;
}

.fork-list {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 12px;
}

.fork-form {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 8px;
}

.fork-form-row {
  display: flex;
  gap: 8px;
}

.fork-port-input {
  max-width: 90px;
  flex: none;
}

.folder-btn {
  display: flex;
  align-items: center;
  gap: 8px;
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
  background: #ffffff;
  cursor: pointer;
  font-size: 13px;
  font-weight: 500;
  color: #3f3f46;
  text-align: left;
  flex: 1;
  min-width: 0;
}

.folder-path {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.field {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 12px;
}

.skill-list {
  display: flex;
  flex-direction: column;
  gap: 6px;
  margin-bottom: 12px;
}

.skill-row {
  border: var(--cerne-border);
  border-radius: 8px;
  padding: 8px 10px;
}

.mcp-row {
  display: flex;
  align-items: center;
  gap: 10px;
}

.mcp-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
  margin-left: auto;
}

.mcp-connector-icon {
  flex-shrink: 0;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.mcp-connector-icon svg {
  width: 20px;
  height: 20px;
}

.mcp-connector-icon-material {
  font-size: 20px;
  color: #71717a;
}

.skill-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.skill-name {
  font-size: 13px;
  font-weight: 600;
  font-family: var(--cerne-mono);
}

.vision-badge {
  display: inline-flex;
  align-items: center;
  margin-left: 4px;
  color: #2563eb;
  vertical-align: middle;
}

.vision-badge .msi {
  font-size: 14px;
}

.vision-checkbox {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
  color: #52525b;
  cursor: pointer;
}

.vision-checkbox input {
  flex-shrink: 0;
}

.skill-desc {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
}

.persona-info {
  min-width: 0;
}

.persona-content-preview {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
  white-space: pre-line;
}

.skill-new {
  display: flex;
  gap: 8px;
  margin-bottom: 8px;
}

.mcp-form {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 8px;
}

.mcp-form-row {
  display: flex;
  gap: 8px;
}

.mcp-env-input {
  resize: vertical;
  font-family: var(--cerne-mono);
  line-height: 1.5;
}

.mcp-remote-toggle {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 600;
  color: #3f3f46;
  white-space: nowrap;
  cursor: pointer;
}

.mcp-form-actions {
  display: flex;
  gap: 8px;
}

.mcp-test-success {
  display: flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
  font-weight: 500;
  color: #16a34a;
  margin: 0 0 8px;
}

.mcp-test-success .msi {
  font-size: 15px;
}

.error-text {
  font-size: 12px;
  font-weight: 500;
  color: #dc2626;
  margin: 0 0 8px;
}

label {
  font-size: 12px;
  font-weight: 600;
  color: #52525b;
}

</style>
