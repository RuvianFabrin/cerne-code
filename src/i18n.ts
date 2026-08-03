import { createI18n } from "vue-i18n";
import ptBR from "./locales/pt-BR.json";
import en from "./locales/en.json";
import zh from "./locales/zh.json";
import es from "./locales/es.json";

export type LocaleCode = "pt-BR" | "en" | "zh" | "es";

export const SUPPORTED_LOCALES: { code: LocaleCode; label: string }[] = [
  { code: "pt-BR", label: "Português (Brasil)" },
  { code: "en", label: "English" },
  { code: "zh", label: "中文" },
  { code: "es", label: "Español" },
];

const STORAGE_KEY = "cerne-locale";

export function loadStoredLocale(): LocaleCode {
  const stored = localStorage.getItem(STORAGE_KEY);
  if (stored && SUPPORTED_LOCALES.some((l) => l.code === stored)) return stored as LocaleCode;
  return "pt-BR";
}

export function persistLocale(locale: LocaleCode) {
  localStorage.setItem(STORAGE_KEY, locale);
}

export const i18n = createI18n({
  legacy: false,
  locale: loadStoredLocale(),
  fallbackLocale: "en",
  messages: {
    "pt-BR": ptBR,
    en,
    zh,
    es,
  },
});

export function setLocale(locale: LocaleCode) {
  i18n.global.locale.value = locale;
  persistLocale(locale);
  document.documentElement.setAttribute("lang", locale);
}
