<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { traceState } from "../composables/useTraceStore";
import PermissionPrompt from "./PermissionPrompt.vue";

const { t } = useI18n();

// Memoise the pending-entries filter so the template doesn't recompute it
// on every render and so the `v-if`/`v-for` agree on the same source.
const pendingEntries = computed(() =>
  traceState.entries.filter(
    (e) => (e.kind === "permission" || e.kind === "memory") && e.status === "pending"
  )
);
</script>

<template>
  <div
    :class="[
      'card',
      'permission-center',
      { 'permission-center--scrollable': pendingEntries.length > 0 }
    ]"
  >
    <div class="card-header">
      <h2>{{ t("permission.panelTitle") }}</h2>
    </div>
    <div class="card-content">
      <KxEmptyState
        v-if="pendingEntries.length === 0"
        class="permission-empty"
        data-test="permission-empty-state"
        compact
      >
        {{ t("permission.emptyState") }}
      </KxEmptyState>
      <ul v-else class="permission-list">
        <li v-for="entry in pendingEntries" :key="entry.id" class="permission-list-item">
          <PermissionPrompt :entry="entry" />
        </li>
      </ul>
    </div>
  </div>
</template>

<style scoped>
.permission-center {
  box-sizing: border-box;
  width: 100%;
  max-width: 100%;
  border-top: 1px solid var(--app-border-color, #d7d7d7);
  max-height: 260px;
  overflow-x: hidden;
  overflow-y: hidden;
}
.permission-center--scrollable {
  overflow-y: auto;
}
.card-header {
  padding: 12px 12px 4px;
}
.card-content {
  box-sizing: border-box;
  padding: 4px 12px 12px;
  max-width: 100%;
}
.permission-center h2 {
  margin: 0;
  font-size: 14px;
}
.permission-empty {
  font-size: 13px;
}
.permission-list {
  list-style: none;
  padding: 0;
  margin: 0;
  max-width: 100%;
}
.permission-list-item {
  padding: 4px 0;
}
.permission-list-item:hover {
  background: var(--app-hover-color, #f0f4f8);
}
</style>
