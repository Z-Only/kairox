<script setup lang="ts">
import ToastContainer from "@/components/ToastContainer.vue";
import ConfirmDialog from "@/components/ConfirmDialog.vue";

const { t } = useI18n();
</script>

<template>
  <ConfirmDialog>
    <div class="app-shell" data-test="app-shell">
      <nav class="app-nav" data-test="app-nav">
        <div class="app-nav__brand" aria-hidden="true">
          <span class="app-nav__mark"></span>
          <span>Kairox</span>
        </div>
        <div class="app-nav__links">
          <RouterLink :to="{ name: 'workbench' }" data-test="nav-workbench">
            {{ t("nav.workbench") }}
          </RouterLink>
          <RouterLink :to="{ name: 'settings-general' }" data-test="nav-settings">
            {{ t("nav.settings") }}
          </RouterLink>
        </div>
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
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  min-height: 44px;
  padding: 6px 16px;
  border-bottom: 1px solid var(--app-border-color);
  background: var(--app-nav-color);
  font-size: 13px;
  box-shadow: var(--app-shadow-sm);
}
.app-nav__brand,
.app-nav__links {
  display: flex;
  align-items: center;
  min-width: 0;
}
.app-nav__brand {
  gap: 8px;
  color: var(--app-text-color);
  font-weight: 750;
}
.app-nav__mark {
  width: 16px;
  height: 16px;
  border: 1px solid color-mix(in srgb, var(--app-primary-color) 45%, var(--app-border-color));
  border-radius: var(--app-radius-sm);
  background:
    linear-gradient(135deg, transparent 44%, var(--app-primary-color) 45% 56%, transparent 57%),
    color-mix(in srgb, var(--app-primary-color) 14%, var(--app-card-color));
}
.app-nav__links {
  gap: 6px;
}
.app-nav a {
  text-decoration: none;
  color: var(--app-text-color-2);
  padding: 6px 12px;
  border-radius: var(--app-radius-md);
  cursor: pointer;
  font-weight: 600;
}
@media (prefers-reduced-motion: no-preference) {
  .app-nav a {
    transition:
      color 0.2s,
      background 0.2s;
  }
}
.app-nav a:hover {
  color: var(--app-text-color);
  background: var(--app-hover-color);
}
.app-nav a.router-link-active {
  color: var(--app-primary-color);
  background: color-mix(in srgb, var(--app-primary-color) 12%, transparent);
}
@media (max-width: 640px) {
  .app-nav {
    align-items: flex-start;
    flex-direction: column;
    gap: 8px;
    padding: 10px 16px;
  }

  .app-nav__links {
    width: 100%;
    overflow-x: auto;
  }
}
</style>
