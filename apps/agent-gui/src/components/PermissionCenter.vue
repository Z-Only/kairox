<script setup lang="ts">
import { computed } from "vue";
import { NCard, NList, NListItem, NEmpty } from "naive-ui";
import { traceState } from "../composables/useTraceStore";
import PermissionPrompt from "./PermissionPrompt.vue";

// Memoise the pending-entries filter so the template doesn't recompute it
// on every render and so the `v-if`/`v-for` agree on the same source.
const pendingEntries = computed(() =>
  traceState.entries.filter(
    (e) =>
      (e.kind === "permission" || e.kind === "memory") && e.status === "pending"
  )
);
</script>

<template>
  <NCard
    size="small"
    class="permission-center"
    :bordered="false"
    content-style="padding: 0;"
  >
    <template #header>
      <h2>Permissions</h2>
    </template>
    <NEmpty
      v-if="pendingEntries.length === 0"
      size="small"
      class="empty-state"
      description="No pending requests"
    />
    <NList v-else hoverable bordered :show-divider="false">
      <NListItem v-for="entry in pendingEntries" :key="entry.id">
        <PermissionPrompt :entry="entry" />
      </NListItem>
    </NList>
  </NCard>
</template>

<style scoped>
.permission-center {
  border-top: 1px solid var(--app-border-color, #d7d7d7);
  max-height: 260px;
  overflow-y: auto;
}
.permission-center :deep(.n-card-header) {
  padding: 12px 12px 4px;
}
.permission-center :deep(.n-card__content) {
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
</style>
