<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { storeToRefs } from "pinia";
import { useUiStore, type ThemeMode, type SupportedLocale } from "@/stores/ui";

// Hoisted to module scope + `as const` so the option arrays are not rebuilt
// per-render and their literal types are preserved through the template.
const themes = [
  { value: "auto", labelKey: "settings.themeAuto" },
  { value: "light", labelKey: "settings.themeLight" },
  { value: "dark", labelKey: "settings.themeDark" }
] as const satisfies ReadonlyArray<{ value: ThemeMode; labelKey: string }>;

const locales = [
  { value: "en", labelKey: "settings.localeEn" },
  { value: "zh-CN", labelKey: "settings.localeZh" }
] as const satisfies ReadonlyArray<{
  value: SupportedLocale;
  labelKey: string;
}>;

const { t } = useI18n();
const ui = useUiStore();
// Read-only refs that drive the `<select>` displayed value. Writes go through
// the `setLocale` / `setTheme` actions in `@change` so the store action is the
// single write path (we do NOT use `v-model` here, which would mutate the
// destructured ref directly and bypass the action).
const { locale, colorMode } = storeToRefs(ui);
</script>

<template>
  <section class="settings" data-test="view-settings">
    <h2>{{ t("settings.title") }}</h2>

    <div class="settings__row">
      <label for="settings-locale">{{ t("settings.locale") }}</label>
      <select
        id="settings-locale"
        :value="locale"
        data-test="settings-locale"
        @change="
          ui.setLocale(
            ($event.target as HTMLSelectElement).value as SupportedLocale
          )
        "
      >
        <option v-for="opt in locales" :key="opt.value" :value="opt.value">
          {{ t(opt.labelKey) }}
        </option>
      </select>
    </div>

    <div class="settings__row">
      <label for="settings-theme">{{ t("settings.theme") }}</label>
      <select
        id="settings-theme"
        :value="colorMode"
        data-test="settings-theme"
        @change="
          ui.setTheme(($event.target as HTMLSelectElement).value as ThemeMode)
        "
      >
        <option v-for="opt in themes" :key="opt.value" :value="opt.value">
          {{ t(opt.labelKey) }}
        </option>
      </select>
    </div>
  </section>
</template>

<style scoped>
.settings {
  padding: 16px;
  max-width: 480px;
}
.settings__row {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-block: 12px;
}
.settings__row label {
  min-width: 100px;
}
</style>
