import { reactive } from "vue";

export const UI_FONT_OPTIONS = [
  { id: "outfit", label: "Outfit", stack: '"Outfit", system-ui, "Segoe UI", sans-serif' },
  { id: "inter", label: "Inter", stack: '"Inter Variable", system-ui, "Segoe UI", sans-serif' },
  { id: "literata", label: "Literata", stack: '"Literata Variable", Georgia, serif' },
  { id: "source-serif-4", label: "Source Serif 4", stack: '"Source Serif 4 Variable", Georgia, serif' },
  { id: "jetbrains-mono", label: "JetBrains Mono", stack: '"JetBrains Mono Variable", Consolas, monospace' },
] as const;

export type UiFontFamily = (typeof UI_FONT_OPTIONS)[number]["id"];

export function isUiFontFamily(value: unknown): value is UiFontFamily {
  return UI_FONT_OPTIONS.some((font) => font.id === value);
}

export type FontSettings = {
  family: UiFontFamily;
  chat: number;
  composer: number;
  zoom: number;
};

export const DEFAULT_FONT_SETTINGS: FontSettings = {
  family: "outfit",
  chat: 16,
  composer: 16,
  zoom: 1,
};

export const FONT_SIZE_LIMITS = {
  chat: { min: 12, max: 20, step: 1 },
  composer: { min: 12, max: 20, step: 1 },
  zoom: { min: 0.8, max: 1.4, step: 0.05 },
} as const;

const STORAGE_KEY = "cerne-font-settings";

function storedTextSize(value: unknown, fallback: number, migrateDefaults: boolean): number {
  if (typeof value !== "number") return fallback;
  // Atualiza o antigo padrão de 14 px uma vez; preserva tamanhos personalizados.
  return migrateDefaults && value === 14 ? fallback : value;
}

function loadStoredFontSettings(): FontSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return { ...DEFAULT_FONT_SETTINGS };
    const parsed = JSON.parse(raw);
    const migrateDefaults = parsed.sizeDefaultsVersion !== 2;
    return {
      family: isUiFontFamily(parsed.family) ? parsed.family : DEFAULT_FONT_SETTINGS.family,
      chat: storedTextSize(parsed.chat, DEFAULT_FONT_SETTINGS.chat, migrateDefaults),
      composer: storedTextSize(parsed.composer, DEFAULT_FONT_SETTINGS.composer, migrateDefaults),
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
  const font = UI_FONT_OPTIONS.find((option) => option.id === fontSettings.family) ?? UI_FONT_OPTIONS[0];
  root.style.setProperty("--cerne-ui", font.stack);
  root.style.setProperty("--cerne-font-chat", `${fontSettings.chat}px`);
  root.style.setProperty("--cerne-font-composer", `${fontSettings.composer}px`);
  root.style.setProperty("--cerne-zoom", String(fontSettings.zoom));
}

function persist() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify({ ...fontSettings, sizeDefaultsVersion: 2 }));
}

export function setFontSetting<K extends keyof FontSettings>(key: K, value: FontSettings[K]) {
  fontSettings[key] = value;
  applyFontSettings();
  persist();
}

export function resetFontSettings() {
  Object.assign(fontSettings, DEFAULT_FONT_SETTINGS);
  applyFontSettings();
  persist();
}
