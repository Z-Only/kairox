<script setup lang="ts">
import { traceState } from "../composables/useTraceStore";
import PermissionPrompt from "./PermissionPrompt.vue";
</script>

<template>
  <section class="permission-center">
    <h2>Permissions</h2>
    <div
      v-if="
        traceState.entries.filter(
          (e) => e.kind === 'permission' && e.status === 'pending'
        ).length === 0 &&
        traceState.entries.filter(
          (e) => e.kind === 'memory' && e.status === 'pending'
        ).length === 0
      "
      class="empty-state"
    >
      No pending requests
    </div>
    <PermissionPrompt
      v-for="entry in traceState.entries.filter(
        (e) =>
          (e.kind === 'permission' || e.kind === 'memory') &&
          e.status === 'pending'
      )"
      :key="entry.id"
      :entry="entry"
    />
  </section>
</template>

<style scoped>
.permission-center {
  padding: 12px;
  border-top: 1px solid #d7d7d7;
  max-height: 260px;
  overflow-y: auto;
}
.permission-center h2 {
  margin: 0 0 8px;
  font-size: 14px;
}
.empty-state {
  color: #999;
  font-size: 13px;
}
</style>
