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
const {
  locale,
  colorMode,
  devtoolsEnabled,
  devtoolsRequiresRestart,
  guiSettingsLoading,
  guiSettingsError
} = storeToRefs(ui);
const isThemeSelectFocused = ref(false);

onMounted(() => {
  void ui.loadGuiSettings();
});

async function onDevtoolsChange(event: Event): Promise<void> {
  const enabled = (event.target as HTMLInputElement).checked;
  try {
    await ui.setDevtoolsEnabled(enabled);
  } catch {
    // The store rolls back and exposes the backend error for the row.
  }
}
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
    <section class="settings__section" aria-labelledby="settings-advanced-title">
      <h2 id="settings-advanced-title" class="settings__section-title">
        {{ t("settings.advanced") }}
      </h2>
      <div class="settings__row" data-test="settings-devtools-row">
        <label for="settings-devtools">{{ t("settings.devtools") }}</label>
        <div class="settings__control-stack">
          <label class="settings__switch">
            <input
              id="settings-devtools"
              data-test="settings-devtools"
              type="checkbox"
              :checked="devtoolsEnabled"
              :disabled="guiSettingsLoading"
              @change="onDevtoolsChange"
            />
            <span class="settings__switch-track" aria-hidden="true">
              <span class="settings__switch-thumb" />
            </span>
          </label>
          <SettingsStatusTag
            v-if="devtoolsRequiresRestart"
            tone="warning"
            data-test="settings-devtools-restart"
          >
            {{ t("settings.restartRequired") }}
          </SettingsStatusTag>
          <small
            v-if="guiSettingsError"
            class="settings__error"
            data-test="settings-devtools-error"
          >
            {{ guiSettingsError }}
          </small>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.general-settings {
  display: grid;
  align-content: start;
  gap: 10px;
}

.settings__section {
  display: grid;
  gap: 10px;
  margin-block-start: 12px;
}

.settings__section-title {
  margin: 0;
  color: var(--app-text-color-2);
  font-size: var(--app-text-sm, 0.875rem);
  font-weight: 700;
}

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

.settings__control-stack {
  display: flex;
  flex: 0 1 auto;
  flex-wrap: wrap;
  gap: 8px;
  align-items: center;
  justify-content: flex-end;
  min-width: 0;
}

.settings__switch {
  position: relative;
  display: inline-flex;
  align-items: center;
  width: 44px;
  height: 26px;
}

.settings__switch input {
  position: absolute;
  width: 1px;
  height: 1px;
  opacity: 0;
}

.settings__switch-track {
  position: relative;
  display: block;
  width: 44px;
  height: 26px;
  border: 1px solid var(--app-border-color);
  border-radius: 999px;
  background: var(--app-muted-surface-color);
  transition:
    background-color 0.16s ease,
    border-color 0.16s ease;
}

.settings__switch-thumb {
  position: absolute;
  top: 3px;
  left: 3px;
  width: 20px;
  height: 20px;
  border-radius: 50%;
  background: var(--app-card-color);
  box-shadow: var(--app-shadow-sm);
  transition: transform 0.16s ease;
}

.settings__switch input:checked + .settings__switch-track {
  border-color: var(--app-primary-color);
  background: var(--app-primary-color);
}

.settings__switch input:checked + .settings__switch-track .settings__switch-thumb {
  transform: translateX(18px);
}

.settings__switch input:focus-visible + .settings__switch-track {
  outline: 2px solid var(--app-primary-color);
  outline-offset: 2px;
}

.settings__switch input:disabled + .settings__switch-track {
  opacity: 0.65;
}

.settings__error {
  flex-basis: 100%;
  color: var(--app-error-color);
  font-size: var(--app-text-xs, 0.75rem);
  text-align: right;
}

@media (max-width: 560px) {
  .settings__row {
    align-items: stretch;
    flex-direction: column;
  }

  .settings__select {
    max-width: none;
  }

  .settings__control-stack {
    justify-content: flex-start;
  }

  .settings__error {
    text-align: left;
  }
}
</style>
