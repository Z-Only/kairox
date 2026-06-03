// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Plain `.ts` infrastructure modules must
// import their dependencies explicitly — otherwise the browser hits
// `Uncaught ReferenceError: createI18n is not defined` and the app
// never mounts.
import { createI18n } from "vue-i18n";
import { watch } from "vue";
import { useUiStore } from "@/stores/ui";
import en from "./en.json";
import zhCN from "./zh-CN.json";
import { readStoredLocalePreference, resolveLocalePreference } from "./localePreference";

export type { SupportedLocale } from "./localePreference";

export const i18n = createI18n({
  legacy: false,
  locale: resolveLocalePreference(readStoredLocalePreference()),
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
 *
 * When locale is "system", we detect the browser's preferred language
 * and use that (falling back to "en" if unsupported).
 */
export function bindLocaleToStore() {
  const ui = useUiStore();
  watch(
    () => ui.locale,
    (next) => {
      i18n.global.locale.value = resolveLocalePreference(next);
    },
    { immediate: true }
  );
}
