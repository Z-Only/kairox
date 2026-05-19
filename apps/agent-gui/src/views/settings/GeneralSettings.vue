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
      <KxSelect
        id="settings-locale"
        :model-value="locale"
        data-test="settings-locale"
        @update:model-value="ui.setLocale($event as SupportedLocale)"
      >
        <option v-for="opt in locales" :key="opt.value" :value="opt.value">
          {{ t(opt.labelKey) }}
        </option>
      </KxSelect>
    </div>
    <div class="settings__row" data-test="theme-toggle">
      <label for="settings-theme">{{ t("settings.theme") }}</label>
      <KxSelect
        id="settings-theme"
        :model-value="colorMode"
        :class="{ 'settings__select--focused': isThemeSelectFocused }"
        data-test="settings-theme"
        @focus="isThemeSelectFocused = true"
        @blur="isThemeSelectFocused = false"
        @update:model-value="ui.setTheme($event as ThemeMode)"
      >
        <option v-for="opt in themes" :key="opt.value" :value="opt.value">
          {{ t(opt.labelKey) }}
        </option>
      </KxSelect>
    </div>
  </div>
</template>

<style scoped>
.settings__row {
  display: flex;
  gap: 12px;
  align-items: center;
  margin-block: 0;
  padding: 12px 0;
  border-bottom: 1px solid var(--app-border-color);
}
.settings__row:last-child {
  border-bottom: none;
}
.settings__row label {
  min-width: 100px;
}
</style>
