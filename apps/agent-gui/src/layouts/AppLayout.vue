<script setup lang="ts">
import { useI18n } from "vue-i18n";
import StatusBar from "@/components/StatusBar.vue";
import NotificationToast from "@/components/NotificationToast.vue";

const { t } = useI18n();
</script>

<template>
  <div class="app-shell" data-test="app-shell">
    <nav class="app-nav" data-test="app-nav">
      <RouterLink to="/workbench" data-test="nav-workbench">
        {{ t("nav.workbench") }}
      </RouterLink>
      <RouterLink to="/marketplace" data-test="nav-marketplace">
        {{ t("nav.marketplace") }}
      </RouterLink>
      <RouterLink to="/settings" data-test="nav-settings">
        {{ t("nav.settings") }}
      </RouterLink>
    </nav>
    <RouterView />
    <StatusBar />
    <NotificationToast />
  </div>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100vh;
}
.app-nav {
  display: flex;
  gap: 8px;
  padding: 6px 12px;
  border-bottom: 1px solid #d7d7d7;
  background: var(--surface-alt, #f7f7f7);
}
.app-nav a {
  padding: 4px 10px;
  border: 1px solid var(--border, #ccc);
  text-decoration: none;
  color: inherit;
  font-size: 12px;
}
.app-nav a.router-link-active {
  background: var(--accent, #345);
  color: #fff;
}
</style>
