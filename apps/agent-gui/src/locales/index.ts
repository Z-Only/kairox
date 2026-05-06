import { createI18n } from "vue-i18n";
import en from "./en.json";
import zhCN from "./zh-CN.json";

export type SupportedLocale = "en" | "zh-CN";

const STORAGE_KEY = "kairox.locale";

function detectInitialLocale(): SupportedLocale {
  if (typeof window === "undefined") return "en";
  const stored = window.localStorage.getItem(STORAGE_KEY);
  return stored === "zh-CN" || stored === "en" ? stored : "en";
}

export const i18n = createI18n({
  legacy: false,
  locale: detectInitialLocale(),
  fallbackLocale: "en",
  messages: { en, "zh-CN": zhCN }
});
