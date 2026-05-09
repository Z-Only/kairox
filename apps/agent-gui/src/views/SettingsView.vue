<script setup lang="ts">
import { useUiStore, type ThemeMode, type SupportedLocale } from "@/stores/ui";
import MarketplacePane from "@/components/MarketplacePane.vue";

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
const activeTab = ref<"general" | "marketplace">("general");
const isThemeSelectFocused = ref(false);
</script>

<template>
  <main class="settings" data-test="view-settings">
    <h1>{{ t("settings.title") }}</h1>

    <div class="tabs" role="tablist">
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'general'"
        @click="activeTab = 'general'"
      >
        {{ t("settings.general") }}
      </button>
      <button
        class="tab-btn"
        role="tab"
        :aria-selected="activeTab === 'marketplace'"
        data-test="settings-tab-marketplace"
        @click="activeTab = 'marketplace'"
      >
        {{ t("nav.marketplace") }}
      </button>
    </div>

    <div v-show="activeTab === 'general'" role="tabpanel">
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

    <div v-show="activeTab === 'marketplace'" role="tabpanel">
      <MarketplacePane />
    </div>
  </main>
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

select:focus,
.settings__select--focused {
  outline: 2px solid var(--app-primary-color, #3b82f6);
  outline-offset: 2px;
  box-shadow: inset 0 0 0 2px var(--app-primary-color, #3b82f6);
  background-color: color-mix(in srgb, var(--app-primary-color, #3b82f6) 12%, transparent);
}

.tabs {
  display: flex;
  gap: 4px;
  border-bottom: 1px solid var(--border-color, #e0e0e0);
  margin-bottom: 12px;
}

.tab-btn {
  padding: 8px 16px;
  border: none;
  background: none;
  cursor: pointer;
  font-size: inherit;
  color: var(--app-text-color-2, #6b7280);
  border-bottom: 2px solid transparent;
  transition:
    color 0.2s,
    border-color 0.2s;
}

.tab-btn[aria-selected="true"] {
  color: var(--primary-color, #18a058);
  border-bottom-color: var(--primary-color, #18a058);
}

.tab-btn:hover {
  color: var(--primary-color, #18a058);
}
</style>
