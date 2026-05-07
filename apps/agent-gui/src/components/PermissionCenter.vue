<script setup lang="ts">
import { traceState } from "../composables/useTraceStore";
import PermissionPrompt from "./PermissionPrompt.vue";

// Memoise the pending-entries filter so the template doesn't recompute it
// on every render and so the `v-if`/`v-for` agree on the same source.
const pendingEntries = computed(() =>
  traceState.entries.filter(
    (e) => (e.kind === "permission" || e.kind === "memory") && e.status === "pending"
  )
);
</script>

<template>
  <div class="card permission-center">
    <div class="card-header">
      <h2>Permissions</h2>
    </div>
    <div class="card-content">
      <div v-if="pendingEntries.length === 0" class="empty-state">No pending requests</div>
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
  border-top: 1px solid var(--app-border-color, #d7d7d7);
  max-height: 260px;
  overflow-y: auto;
}
.card-header {
  padding: 12px 12px 4px;
}
.card-content {
  padding: 4px 12px 12px;
}
.permission-center h2 {
  margin: 0;
  font-size: 14px;
}
.empty-state {
  color: var(--app-text-disabled-color, #999);
  font-size: 13px;
}
.permission-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.permission-list-item {
  padding: 4px 0;
}
.permission-list-item:hover {
  background: var(--app-hover-color, #f0f4f8);
}
</style>
