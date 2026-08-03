<script setup lang="ts">
import { computed, onMounted, ref } from "vue";
import { useI18n } from "vue-i18n";
import { open } from "@tauri-apps/plugin-dialog";
import { api, type CustomProviderConfig, type McpServerConfig, type ProviderKind, type SearchProviderKind, type SkillMeta } from "../api";
import { PROVIDER_KINDS, providerLabel, useProviderStore } from "../stores/provider";
import { SUPPORTED_LOCALES, setLocale, type LocaleCode } from "../i18n";
import LlamaForkRow from "./LlamaForkRow.vue";
import ModelBrowserDialog from "./ModelBrowserDialog.vue";
import SkillEditorModal from "./SkillEditorModal.vue";

const { t, locale } = useI18n();
const providerStore = useProviderStore();
const openrouterKeyInput = ref("");

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
  const selected = await open({ directory: false, multiple: false, filters: [{ name: t("settings.executableFilter"), extensions: ["exe"] }] });
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

const skills = ref<SkillMeta[]>([]);
const skillModalVisible = ref(false);
const skillModalTarget = ref<SkillMeta | null>(null);

async function loadSkills() {
  skills.value = await api.listSkills(null);
}

function openSkillModal(skill: SkillMeta | null) {
  skillModalTarget.value = skill;
  skillModalVisible.value = true;
}

const mcpServers = ref<McpServerConfig[]>([]);
const newMcpName = ref("");
const newMcpCommand = ref("");
const newMcpArgs = ref("");
const newMcpEnv = ref("");
const mcpError = ref("");
// null = criando um servidor novo; preenchido = editando um já existente (o
// campo nome vira somente-leitura, já que é a chave que identifica o
// servidor — trocar o nome aqui criaria um servidor novo em vez de editar).
const editingMcpName = ref<string | null>(null);

type McpTestStatus = "idle" | "testing" | "success" | "error";
const mcpTestStatus = ref<McpTestStatus>("idle");
const mcpTestTools = ref<string[]>([]);
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
  if (!name || !newMcpCommand.value.trim()) return null;
  const existing = mcpServers.value.find((s) => s.name === name);
  return {
    name,
    command: newMcpCommand.value.trim(),
    args: newMcpArgs.value.trim().length > 0 ? newMcpArgs.value.trim().split(/\s+/) : [],
    env: parseEnvLines(newMcpEnv.value),
    enabled: existing?.enabled ?? true,
  };
}

function resetMcpForm() {
  editingMcpName.value = null;
  newMcpName.value = "";
  newMcpCommand.value = "";
  newMcpArgs.value = "";
  newMcpEnv.value = "";
  mcpTestStatus.value = "idle";
  mcpTestTools.value = [];
  mcpTestError.value = "";
}

function startEditMcpServer(server: McpServerConfig) {
  editingMcpName.value = server.name;
  newMcpName.value = server.name;
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
]);

const searchProvider = ref<SearchProviderKind>("auto");
const searchSearxngUrl = ref("http://127.0.0.1:8888");
const searchApiKeyInput = ref("");
const searchHasBraveKey = ref(false);
const searchHasTavilyKey = ref(false);
const searchError = ref("");
type SearchTestStatus = "idle" | "testing" | "success" | "error";
const searchTestStatus = ref<SearchTestStatus>("idle");
const searchTestCount = ref(0);
const searchTestError = ref("");

async function loadSearchConfig() {
  const cfg = await api.getSearchConfig();
  searchProvider.value = cfg.provider;
  searchSearxngUrl.value = cfg.searxng_url;
  searchHasBraveKey.value = cfg.has_brave_key;
  searchHasTavilyKey.value = cfg.has_tavily_key;
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
    );
    searchHasBraveKey.value = cfg.has_brave_key;
    searchHasTavilyKey.value = cfg.has_tavily_key;
    searchApiKeyInput.value = "";
  } catch (e) {
    searchError.value = String(e);
  }
}

async function clearSearchApiKey() {
  const cfg = await api.clearSearchApiKey(searchProvider.value);
  searchHasBraveKey.value = cfg.has_brave_key;
  searchHasTavilyKey.value = cfg.has_tavily_key;
}

onMounted(() => {
  loadSkills();
  loadMcpServers();
  loadSearchConfig();
});

async function saveKey() {
  if (!openrouterKeyInput.value.trim()) return;
  await providerStore.saveOpenrouterKey(openrouterKeyInput.value.trim());
  openrouterKeyInput.value = "";
}

</script>

<template>
  <div class="settings">
    <div class="settings-inner">
      <h1>{{ $t("settings.title") }}</h1>

      <section>
        <h2>{{ $t("settings.language") }}</h2>
        <p class="hint">{{ $t("settings.languageHint") }}</p>
        <select :value="locale" class="text-input" @change="onLocaleChange(($event.target as HTMLSelectElement).value)">
          <option v-for="l in SUPPORTED_LOCALES" :key="l.code" :value="l.code">{{ l.label }}</option>
        </select>
      </section>

      <section>
        <h2>{{ $t("settings.activeProvider") }}</h2>
        <p class="hint">{{ $t("settings.activeProviderHint") }}</p>
        <div class="provider-grid">
          <button
            v-for="kind in PROVIDER_KINDS"
            :key="kind"
            class="provider-card"
            :class="{ active: providerStore.config?.active_provider === kind }"
            @click="providerStore.setActiveProvider(kind as any)"
          >
            {{ providerLabel(kind) }}
          </button>
        </div>
      </section>

      <section>
        <h2>OpenRouter</h2>
        <p class="hint">
          {{ $t("settings.apiKeyVaultHint") }}
          <strong>{{ providerStore.hasOpenrouterKey ? $t("settings.keyConfigured") : $t("settings.noKeySaved") }}</strong>
        </p>
        <div class="key-row">
          <input v-model="openrouterKeyInput" type="password" placeholder="sk-or-..." class="text-input" />
          <button class="btn-primary" @click="saveKey">{{ $t("sidebar.save") }}</button>
        </div>
        <button class="btn-secondary browse-models-btn" @click="openModelBrowser('openrouter', $t('settings.modelsOpenRouterTitle'))">
          <span class="msi">search</span>
          {{ $t("settings.viewModels") }}
        </button>
      </section>

      <section>
        <h2>{{ $t("settings.llamaCppLocal") }}</h2>
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
              <span class="folder-path">{{ newForkExe || "llama-server.exe..." }}</span>
            </button>
            <button class="folder-btn" @click="pickForkIni">
              <span class="msi">folder_open</span>
              <span class="folder-path">{{ newForkIni || "models.ini..." }}</span>
            </button>
          </div>
          <button class="btn-primary" @click="addFork">{{ $t("settings.addFork") }}</button>
        </div>
        <p v-if="forkError" class="error-text">{{ forkError }}</p>
      </section>

      <section>
        <h2>{{ $t("settings.customProviders") }}</h2>
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
      </section>

      <section>
        <h2>{{ $t("settings.skills") }}</h2>
        <p class="hint" v-html="$t('settings.skillsHint')"></p>
        <div class="skill-list">
          <div v-for="skill in skills" :key="skill.dir" class="skill-row mcp-row">
            <div class="skill-info">
              <span class="skill-name">{{ skill.name }}</span>
              <span class="skill-desc">{{ skill.description }}</span>
            </div>
            <div class="mcp-actions">
              <button class="btn-secondary" @click="openSkillModal(skill)">{{ $t("settings.edit") }}</button>
            </div>
          </div>
          <p v-if="skills.length === 0" class="hint">{{ $t("settings.noSkillsYet") }}</p>
        </div>
        <div class="skill-new">
          <button class="btn-primary" @click="openSkillModal(null)">{{ $t("settings.createSkill") }}</button>
          <button class="btn-secondary" @click="api.openSkillsFolder()">{{ $t("settings.openSkillsFolder") }}</button>
        </div>
      </section>

      <SkillEditorModal v-model:visible="skillModalVisible" :skill="skillModalTarget" @saved="loadSkills" />

      <section>
        <h2>{{ $t("settings.mcpServers") }}</h2>
        <p class="hint">
          {{ $t("settings.mcpHintBefore") }}
          <code>mcp__{{ '{servidor}' }}__{{ '{tool}' }}</code>{{ $t("settings.mcpHintAfter") }}
          <code>mcp_servers.json</code>{{ $t("settings.mcpHintAfter2") }}
          <code>mcpServers</code>{{ $t("settings.mcpHintAfter3") }}
        </p>
        <div class="skill-list">
          <div v-for="server in mcpServers" :key="server.name" class="skill-row mcp-row">
            <div class="skill-info">
              <span class="skill-name">{{ server.name }}</span>
              <span class="skill-desc">{{ server.command }} {{ server.args.join(" ") }}</span>
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
        <div class="mcp-form">
          <div class="mcp-form-row">
            <input
              v-model="newMcpName"
              class="text-input"
              placeholder="nome (ex: github)"
              :disabled="!!editingMcpName"
              v-tooltip.top="editingMcpName ? $t('settings.nameCannotChange') : ''"
            />
            <input v-model="newMcpCommand" class="text-input" placeholder="comando (ex: npx)" />
          </div>
          <input v-model="newMcpArgs" class="text-input" placeholder="argumentos (ex: -y @escopo/pacote)" />
          <textarea
            v-model="newMcpEnv"
            class="text-input mcp-env-input"
            rows="2"
            :placeholder="$t('settings.envVarsPlaceholder')"
          />
          <div class="mcp-form-actions">
            <button class="btn-secondary" :disabled="mcpTestStatus === 'testing'" @click="testMcpServer">
              {{ mcpTestStatus === "testing" ? $t("settings.testing") : $t("settings.testConnection") }}
            </button>
            <button class="btn-primary" @click="saveMcpServer">{{ editingMcpName ? $t("sidebar.save") : $t("settings.add") }}</button>
            <button v-if="editingMcpName" class="btn-secondary" @click="resetMcpForm">{{ $t("newSession.cancel") }}</button>
          </div>
          <p v-if="mcpTestStatus === 'success'" class="mcp-test-success">
            <span class="msi">check_circle</span>
            {{ $t("settings.connectedToolsFound", { count: mcpTestTools.length, list: mcpTestTools.length ? ": " + mcpTestTools.join(", ") : "" }) }}
          </p>
          <p v-if="mcpTestStatus === 'error'" class="error-text">{{ mcpTestError }}</p>
        </div>
        <p v-if="mcpError" class="error-text">{{ mcpError }}</p>
      </section>

      <section>
        <h2>{{ $t("settings.webSearch") }}</h2>
        <p class="hint" v-html="$t('settings.webSearchHint')"></p>
        <div class="field">
          <label>Provider</label>
          <select v-model="searchProvider" class="text-input">
            <option v-for="opt in SEARCH_PROVIDER_OPTIONS" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
          </select>
        </div>
        <div v-if="searchProvider === 'brave' || searchProvider === 'tavily'" class="field">
          <label>{{ $t("settings.apiKey") }}</label>
          <input
            v-model="searchApiKeyInput"
            type="password"
            class="text-input"
            :placeholder="
              (searchProvider === 'brave' ? searchHasBraveKey : searchHasTavilyKey)
                ? $t('settings.keyAlreadyConfigured')
                : $t('settings.apiKeyPlaceholder')
            "
          />
          <button
            v-if="(searchProvider === 'brave' && searchHasBraveKey) || (searchProvider === 'tavily' && searchHasTavilyKey)"
            class="btn-secondary"
            @click="clearSearchApiKey"
          >
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
      </section>

      <section v-if="providerStore.config">
        <h2>{{ $t("settings.localEndpoints") }}</h2>
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
      </section>
    </div>

    <ModelBrowserDialog
      v-model:visible="modelBrowser.visible"
      :kind="modelBrowser.kind"
      :fork-id="modelBrowser.forkId"
      :custom-provider-id="modelBrowser.customProviderId"
      :title="modelBrowser.title"
    />
  </div>
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

h2 {
  font-size: 14px;
  font-weight: 700;
  margin: 0 0 4px;
}

section {
  margin-bottom: 28px;
  padding-bottom: 24px;
  border-bottom: var(--cerne-border);
}

.hint {
  font-size: 12px;
  font-weight: 500;
  color: #71717a;
  margin: 0 0 12px;
}

.provider-grid {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 8px;
}

.provider-card {
  border: var(--cerne-border);
  border-radius: 10px;
  padding: 12px;
  background: #ffffff;
  cursor: pointer;
  font-size: 13px;
  font-weight: 600;
  color: #3f3f46;
  text-align: left;
}

.provider-card.active {
  border-color: #18181b;
  background: #fafafa;
  color: #18181b;
}

.key-row {
  display: flex;
  gap: 8px;
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
  justify-content: space-between;
  gap: 8px;
}

.mcp-actions {
  display: flex;
  gap: 6px;
  flex-shrink: 0;
}

.skill-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.skill-name {
  font-size: 13px;
  font-weight: 600;
  font-family: ui-monospace, monospace;
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
  font-family: ui-monospace, monospace;
  line-height: 1.5;
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
