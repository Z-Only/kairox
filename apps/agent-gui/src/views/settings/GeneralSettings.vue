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
  <div class="general-settings" role="tabpanel">
    <div class="settings__row">
      <label for="settings-locale">{{ t("settings.locale") }}</label>
      <KxSelect
        id="settings-locale"
        :model-value="locale"
        class="settings__select"
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
        :class="['settings__select', { 'settings__select--focused': isThemeSelectFocused }]"
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
  justify-content: space-between;
  max-width: 520px;
  margin-block: 0 10px;
  padding: 14px 16px;
  border: 1px solid var(--app-border-color);
  border-radius: var(--app-radius-lg);
  background: var(--app-card-color);
  box-shadow: var(--app-shadow-sm);
}
.settings__row label {
  min-width: 100px;
  color: var(--app-text-color);
  font-weight: 650;
}
.settings__select {
  flex: 0 1 160px;
  max-width: 160px;
  text-align: center;
}
@media (max-width: 560px) {
  .settings__row {
    align-items: stretch;
    flex-direction: column;
  }

  .settings__select {
    max-width: none;
  }
}
</style>
