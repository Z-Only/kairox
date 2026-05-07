<script setup lang="ts">
import { darkTheme, useThemeVars, type GlobalTheme } from "naive-ui";
import { useUiStore } from "@/stores/ui";
import { lightThemeOverrides, darkThemeOverrides } from "@/styles/naive-theme";
import NotificationToast from "@/components/NotificationToast.vue";

const { t } = useI18n();
const ui = useUiStore();
const { isDark } = storeToRefs(ui);

// `null` selects NaiveUI's default light theme. We avoid importing
// `lightTheme` explicitly because passing `null` is the documented way to opt
// out of any built-in theme and is what the framework treats as "light".
const theme = computed<GlobalTheme | null>(() => (isDark.value ? darkTheme : null));
const themeOverrides = computed(() => (isDark.value ? darkThemeOverrides : lightThemeOverrides));

// `useThemeVars()` is the documented way to read NaiveUI theme tokens from
// outside a NaiveUI component. We expose a stable subset as `--app-*` CSS
// custom properties on the shell root so the scoped CSS below (and any
// non-NaiveUI descendant) can reference them and stay in sync with light/dark
// mode automatically. NaiveUI's internal `--n-*` variables are
// component-scoped and not part of the public API — never reference them.
const themeVars = useThemeVars();
</script>

<template>
  <NConfigProvider :theme="theme" :theme-overrides="themeOverrides">
    <NLoadingBarProvider>
      <NMessageProvider>
        <NDialogProvider>
          <NNotificationProvider>
            <div
              class="app-shell"
              data-test="app-shell"
              :style="{
                '--app-body-color': themeVars.bodyColor,
                '--app-card-color': themeVars.cardColor,
                '--app-border-color': themeVars.borderColor,
                '--app-text-color': themeVars.textColor1,
                '--app-primary-color': themeVars.primaryColor,
                '--app-text-color-2': themeVars.textColor2,
                '--app-text-color-3': themeVars.textColor3,
                '--app-success-color': themeVars.successColor,
                '--app-warning-color': themeVars.warningColor,
                '--app-error-color': themeVars.errorColor,
                '--app-info-color': themeVars.infoColor,
                '--app-hover-color': themeVars.hoverColor,
                '--app-code-bg': themeVars.codeColor
              }"
            >
              <nav class="app-nav" data-test="app-nav">
                <RouterLink :to="{ name: 'workbench' }" data-test="nav-workbench">
                  {{ t("nav.workbench") }}
                </RouterLink>
                <RouterLink :to="{ name: 'settings' }" data-test="nav-settings">
                  {{ t("nav.settings") }}
                </RouterLink>
              </nav>
              <RouterView />
              <NotificationToast />
            </div>
          </NNotificationProvider>
        </NDialogProvider>
      </NMessageProvider>
    </NLoadingBarProvider>
  </NConfigProvider>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
}
.app-nav {
  display: flex;
  gap: 16px;
  align-items: center;
  padding: 6px 16px;
  border-bottom: 1px solid var(--app-border-color);
  background: var(--app-card-color);
  font-size: 13px;
}
.app-nav a {
  text-decoration: none;
  color: var(--app-text-color-2);
  padding: 4px 8px;
  border-radius: 4px;
  transition:
    color 0.2s,
    background 0.2s;
}
.app-nav a:hover {
  color: var(--app-text-color);
  background: var(--app-hover-color);
}
.app-nav a.router-link-active {
  color: var(--app-primary-color);
  font-weight: 600;
}
</style>
