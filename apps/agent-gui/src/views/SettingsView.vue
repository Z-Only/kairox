<script setup lang="ts">
import { useUiStore, type ThemeMode, type SupportedLocale } from "@/stores/ui";
import { NSelect, NTabs, NTabPane } from "naive-ui";

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
const { locale, colorMode } = storeToRefs(ui);
</script>

<template>
  <section class="settings" data-test="view-settings">
    <h2>{{ t("settings.title") }}</h2>

    <NTabs type="line" animated>
      <NTabPane name="general" :tab="t('settings.general')">
        <div class="settings__row">
          <label for="settings-locale">{{ t("settings.locale") }}</label>
          <NSelect
            id="settings-locale"
            :value="locale"
            :options="locales.map((opt) => ({ label: t(opt.labelKey), value: opt.value }))"
            data-test="settings-locale"
            @update:value="ui.setLocale"
          />
        </div>

        <div class="settings__row">
          <label for="settings-theme">{{ t("settings.theme") }}</label>
          <NSelect
            id="settings-theme"
            :value="colorMode"
            :options="themes.map((opt) => ({ label: t(opt.labelKey), value: opt.value }))"
            data-test="settings-theme"
            @update:value="ui.setTheme"
          />
        </div>
      </NTabPane>

      <NTabPane name="marketplace" :tab="t('nav.marketplace')">
        <RouterView />
      </NTabPane>
    </NTabs>
  </section>
</template>

<style scoped>
.settings {
  padding: 16px;
  max-width: 640px;
  flex: 1;
  overflow: auto;
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
