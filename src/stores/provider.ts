import { defineStore } from "pinia";
import {
  api,
  type AppConfig,
  type CustomProviderConfig,
  type LlamaForkConfig,
  type ModelInfo,
  type ProviderKind,
} from "../api";

export const PROVIDER_LABELS: Record<ProviderKind, string> = {
  openrouter: "OpenRouter",
  llama_cpp: "llama.cpp local",
  ollama: "Ollama",
  lm_studio: "LM Studio",
  custom: "Customizado",
};

// Modelos são cacheados por provider+conexão específica — pra "llama_cpp" a
// chave é o fork, pra "custom" é o id do provider customizado (senão trocar
// de conexão custom não invalidaria a lista antiga). Providers sem essa
// noção (Ollama/OpenRouter/LM Studio) usam a própria `kind` como chave.
function modelsCacheKey(kind: ProviderKind, forkId?: string, customProviderId?: string): string {
  if (kind === "llama_cpp") return `llama_cpp:${forkId ?? ""}`;
  if (kind === "custom") return `custom:${customProviderId ?? ""}`;
  return kind;
}

export const useProviderStore = defineStore("provider", {
  state: () => ({
    config: null as AppConfig | null,
    models: {} as Record<string, ModelInfo[]>,
    modelsLoading: {} as Record<string, boolean>,
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
