import { reactive } from "vue";

export type FontSettings = {
  chat: number;
  composer: number;
  zoom: number;
};

export const DEFAULT_FONT_SETTINGS: FontSettings = {
  chat: 14,
  composer: 14,
  zoom: 1,
};

export const FONT_SIZE_LIMITS = {
  chat: { min: 12, max: 20, step: 1 },
  composer: { min: 12, max: 20, step: 1 },
  zoom: { min: 0.8, max: 1.4, step: 0.05 },
} as const;

const STORAGE_KEY = "cerne-font-settings";

function loadStoredFontSettings(): FontSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_FONT_SETTINGS };
    const parsed = JSON.parse(raw);
    return {
      chat: typeof parsed.chat === "number" ? parsed.chat : DEFAULT_FONT_SETTINGS.chat,
      composer: typeof parsed.composer === "number" ? parsed.composer : DEFAULT_FONT_SETTINGS.composer,
      zoom: typeof parsed.zoom === "number" ? parsed.zoom : DEFAULT_FONT_SETTINGS.zoom,
    };
  } catch {
    return { ...DEFAULT_FONT_SETTINGS };
  }
}

export const fontSettings = reactive<FontSettings>(loadStoredFontSettings());

// Aplica como CSS custom properties na raiz — `--cerne-zoom` usa a propriedade
// `zoom` (suportada no WebView2/Chromium do Windows e no WebKit atual) pra
// escalar a interface toda de uma vez, já que a maioria dos componentes usa
// font-size em px fixo em vez de rem/em.
export function applyFontSettings() {
  const root = document.documentElement;
  root.style.setProperty("--cerne-font-chat", `${fontSettings.chat}px`);
  root.style.setProperty("--cerne-font-composer", `${fontSettings.composer}px`);
  root.style.setProperty("--cerne-zoom", String(fontSettings.zoom));
}

function persist() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(fontSettings));
}

export function setFontSetting<K extends keyof FontSettings>(key: K, value: FontSettings[K]) {
  fontSettings[key] = value;
  applyFontSettings();
  persist();
}

export function resetFontSettings() {
  (Object.keys(DEFAULT_FONT_SETTINGS) as (keyof FontSettings)[]).forEach((key) => {
    fontSettings[key] = DEFAULT_FONT_SETTINGS[key];
  });
  applyFontSettings();
  persist();
}
