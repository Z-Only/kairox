<script setup lang="ts">
import ToastContainer from "@/components/ToastContainer.vue";
import ConfirmDialog from "@/components/ConfirmDialog.vue";

const { t } = useI18n();
</script>

<template>
  <ConfirmDialog>
    <div class="app-shell" data-test="app-shell">
      <nav class="app-nav" data-test="app-nav">
        <RouterLink :to="{ name: 'workbench' }" data-test="nav-workbench">
          {{ t("nav.workbench") }}
        </RouterLink>
        <RouterLink :to="{ name: 'settings' }" data-test="nav-settings">
          {{ t("nav.settings") }}
        </RouterLink>
      </nav>
      <RouterView />
    </div>
    <ToastContainer />
  </ConfirmDialog>
</template>

<style scoped>
.app-shell {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
  overflow: hidden;
  background: var(--app-body-color);
  color: var(--app-text-color);
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
