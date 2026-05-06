<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { storeToRefs } from "pinia";
import { useUiStore, type ThemeMode, type SupportedLocale } from "@/stores/ui";

const { t } = useI18n();
const ui = useUiStore();
const { locale, colorMode } = storeToRefs(ui);

const themes: { value: ThemeMode; labelKey: string }[] = [
  { value: "auto", labelKey: "settings.themeAuto" },
  { value: "light", labelKey: "settings.themeLight" },
  { value: "dark", labelKey: "settings.themeDark" }
];

const locales: { value: SupportedLocale; labelKey: string }[] = [
  { value: "en", labelKey: "settings.localeEn" },
  { value: "zh-CN", labelKey: "settings.localeZh" }
];
</script>

<template>
  <section class="settings" data-test="view-settings">
    <h2>{{ t("settings.title") }}</h2>

    <div class="settings__row">
      <label>{{ t("settings.locale") }}</label>
      <select
        v-model="locale"
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
      <label>{{ t("settings.theme") }}</label>
      <select
        v-model="colorMode"
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
