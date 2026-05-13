<script setup lang="ts">
import { useUiStore, type ThemeMode, type SupportedLocale } from "@/stores/ui";

const themes = [
  { value: "auto", labelKey: "settings.themeAuto" },
  { value: "light", labelKey: "settings.themeLight" },
  { value: "dark", labelKey: "settings.themeDark" }
] as const satisfies ReadonlyArray<{ value: ThemeMode; labelKey: string }>;

const locales = [
  { value: "system", labelKey: "settings.localeSystem" },
  { value: "en", labelKey: "settings.localeEn" },
  { value: "zh-CN", labelKey: "settings.localeZh" }
] as const satisfies ReadonlyArray<{ value: SupportedLocale; labelKey: string }>;

const { t } = useI18n();
const ui = useUiStore();
const { locale, colorMode } = storeToRefs(ui);
const isThemeSelectFocused = ref(false);
</script>

<template>
  <div role="tabpanel">
    <div class="settings__row">
      <label for="settings-locale">{{ t("settings.locale") }}</label>
      <select
        id="settings-locale"
        :value="locale"
        data-test="settings-locale"
        @change="ui.setLocale(($event.target as HTMLSelectElement).value as SupportedLocale)"
      >
        <option v-for="opt in locales" :key="opt.value" :value="opt.value">
          {{ t(opt.labelKey) }}
        </option>
      </select>
    </div>
    <div class="settings__row" data-test="theme-toggle">
      <label for="settings-theme">{{ t("settings.theme") }}</label>
      <select
        id="settings-theme"
        :value="colorMode"
        :class="{ 'settings__select--focused': isThemeSelectFocused }"
        data-test="settings-theme"
        @focus="isThemeSelectFocused = true"
        @blur="isThemeSelectFocused = false"
        @change="ui.setTheme(($event.target as HTMLSelectElement).value as ThemeMode)"
      >
        <option v-for="opt in themes" :key="opt.value" :value="opt.value">
          {{ t(opt.labelKey) }}
        </option>
      </select>
    </div>
  </div>
</template>

<style scoped>
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
