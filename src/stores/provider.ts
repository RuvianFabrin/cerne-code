import { defineStore } from "pinia";
import {
  api,
  type AppConfig,
  type CustomProviderConfig,
  type LlamaForkConfig,
  type ModelInfo,
  type ProviderKind,
} from "../api";
import { i18n } from "../i18n";

export const PROVIDER_KINDS: ProviderKind[] = ["openrouter", "llama_cpp", "ollama", "lm_studio", "custom"];

// Funcao (nao objeto estatico) pra reagir a troca de idioma - chamada de
// dentro de computed/template, entao o acesso a `i18n.global.locale`
// dentro de `t()` registra a dependencia reativa normalmente.
export function providerLabel(kind: ProviderKind): string {
  return i18n.global.t(`providers.${kind}`);
}

// Modelos são cacheados por provider+conexão específica — pra "llama_cpp" a
// chave é o fork, pra "custom" é o id do provider customizado (senão trocar
// de conexão custom não invalidaria a lista antiga). Providers sem essa
// noção (Ollama/OpenRouter/LM Studio) usam a própria `kind` como chave.
function modelsCacheKey(kind: ProviderKind, forkId?: string, customProviderId?: string): string {
  if (kind === "llama_cpp") return `llama_cpp:${forkId ?? ""}`;
  if (kind === "custom") return `custom:${customProviderId ?? ""}`;
  return kind;
}

// Quantos modelos mostrar no dropdown de modelo quando NÃO há favoritos —
// o OpenRouter tem centenas e renderizar todos no Select pesa; 100 mantém a
// busca útil sem travar. Com favoritos marcados, o dropdown mostra só eles.
const MODEL_DROPDOWN_LIMIT = 100;

export const useProviderStore = defineStore("provider", {
  state: () => ({
    config: null as AppConfig | null,
    models: {} as Record<string, ModelInfo[]>,
    modelsLoading: {} as Record<string, boolean>,
    favorites: {} as Record<string, string[]>,
    favoritesLoaded: {} as Record<string, boolean>,
    forks: [] as LlamaForkConfig[],
    customProviders: [] as CustomProviderConfig[],
    hasOpenrouterKey: false,
    loading: false,
    error: "",
  }),
  actions: {
    async init() {
      this.loading = true;
      try {
        this.config = await api.getConfig();
        this.hasOpenrouterKey = await api.hasOpenrouterKey();
        this.forks = await api.listLlamaForks();
        this.customProviders = await api.listCustomProviders();
        await this.refreshModels(this.config.active_provider, undefined, this.config.active_custom_provider_id ?? undefined);
      } catch (e) {
        this.error = String(e);
      } finally {
        this.loading = false;
      }
    },
    async refreshModels(kind: ProviderKind, forkId?: string, customProviderId?: string) {
      const key = modelsCacheKey(kind, forkId, customProviderId);
      this.modelsLoading[key] = true;
      try {
        const models =
          kind === "llama_cpp"
            ? await api.listLlamaPresets(forkId ?? this.config?.active_llama_fork ?? "turboquant")
            : await api.listProviderModels(kind, customProviderId);
        this.models[key] = models;
        this.error = "";
      } catch (e) {
        this.models[key] = [];
        this.error = String(e);
      } finally {
        this.modelsLoading[key] = false;
      }
    },
    modelsFor(kind: ProviderKind, forkId?: string, customProviderId?: string): ModelInfo[] {
      return this.models[modelsCacheKey(kind, forkId, customProviderId)] ?? [];
    },
    modelsLoadingFor(kind: ProviderKind, forkId?: string, customProviderId?: string): boolean {
      return this.modelsLoading[modelsCacheKey(kind, forkId, customProviderId)] ?? false;
    },
    async loadFavorites(kind: ProviderKind, forkId?: string, customProviderId?: string) {
      const key = modelsCacheKey(kind, forkId, customProviderId);
      if (this.favoritesLoaded[key]) return;
      try {
        this.favorites[key] = await api.getModelFavorites(key);
      } catch {
        this.favorites[key] = [];
      } finally {
        this.favoritesLoaded[key] = true;
      }
    },
    favoritesFor(kind: ProviderKind, forkId?: string, customProviderId?: string): string[] {
      return this.favorites[modelsCacheKey(kind, forkId, customProviderId)] ?? [];
    },
    isFavorite(kind: ProviderKind, modelId: string, forkId?: string, customProviderId?: string): boolean {
      return this.favoritesFor(kind, forkId, customProviderId).includes(modelId);
    },
    async toggleFavorite(kind: ProviderKind, modelId: string, forkId?: string, customProviderId?: string) {
      const key = modelsCacheKey(kind, forkId, customProviderId);
      const current = this.favorites[key] ?? [];
      const next = current.includes(modelId)
        ? current.filter((id) => id !== modelId)
        : [...current, modelId];
      this.favorites[key] = next;
      this.favoritesLoaded[key] = true;
      await api.setModelFavorites(key, next);
    },
    // O que o dropdown de modelo mostra: só os favoritos (na ordem em que
    // foram marcados) quando houver algum; senão os primeiros
    // MODEL_DROPDOWN_LIMIT pra lista gigante (OpenRouter) não travar o Select.
    visibleModelsFor(kind: ProviderKind, forkId?: string, customProviderId?: string): ModelInfo[] {
      const all = this.modelsFor(kind, forkId, customProviderId);
      const favIds = this.favoritesFor(kind, forkId, customProviderId);
      if (favIds.length > 0) {
        const byId = new Map(all.map((m) => [m.id, m]));
        return favIds.map((id) => byId.get(id)).filter((m): m is ModelInfo => !!m);
      }
      return all.slice(0, MODEL_DROPDOWN_LIMIT);
    },
    async setActiveProvider(kind: ProviderKind) {
      if (!this.config) return;
      this.config.active_provider = kind;
      this.config.active_model = null;
      await api.setConfig(this.config);
      await this.refreshModels(kind, undefined, this.config.active_custom_provider_id ?? undefined);
    },
    async setActiveModel(id: string) {
      if (!this.config) return;
      this.config.active_model = id;
      await api.setConfig(this.config);
    },
    async setActiveLlamaFork(forkId: string) {
      if (!this.config) return;
      this.config.active_llama_fork = forkId;
      await api.setConfig(this.config);
      await this.refreshModels("llama_cpp", forkId);
    },
    async setActiveCustomProvider(id: string) {
      if (!this.config) return;
      this.config.active_custom_provider_id = id;
      await api.setConfig(this.config);
      await this.refreshModels("custom", undefined, id);
    },
    async saveOpenrouterKey(key: string) {
      await api.setOpenrouterKey(key);
      this.hasOpenrouterKey = true;
    },
    async saveConfig() {
      if (!this.config) return;
      await api.setConfig(this.config);
    },
    async addLlamaFork(fork: LlamaForkConfig) {
      this.forks = await api.addLlamaFork(fork);
    },
    async removeLlamaFork(id: string) {
      this.forks = await api.removeLlamaFork(id);
    },
    async addCustomProvider(provider: CustomProviderConfig, apiKey?: string) {
      this.customProviders = await api.addCustomProvider(provider, apiKey);
    },
    async removeCustomProvider(id: string) {
      this.customProviders = await api.removeCustomProvider(id);
    },
  },
});
