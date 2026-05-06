import { createI18n } from "vue-i18n";
import { watch } from "vue";
import { useUiStore } from "@/stores/ui";
import en from "./en.json";
import zhCN from "./zh-CN.json";

export type SupportedLocale = "en" | "zh-CN";

function detectInitialLocale(): SupportedLocale {
  if (typeof window === "undefined") return "en";
  const stored = window.localStorage.getItem("kairox.locale");
  return stored === "zh-CN" || stored === "en" ? stored : "en";
}

export const i18n = createI18n({
  legacy: false,
  locale: detectInitialLocale(),
  fallbackLocale: "en",
  messages: { en, "zh-CN": zhCN }
});

/**
 * Wire the ui store's `locale` ref to i18n's runtime locale.
 * Must be called after `app.use(createPinia())` runs.
 *
 * Both i18n and ui.locale are seeded from the same localStorage key
 * ("kairox.locale"); here we only set up the one-way binding
 * ui.locale → i18n.global.locale.
 */
export function bindLocaleToStore() {
  const ui = useUiStore();
  watch(
    () => ui.locale,
    (next) => {
      i18n.global.locale.value = next;
    },
    { immediate: true }
  );
}
