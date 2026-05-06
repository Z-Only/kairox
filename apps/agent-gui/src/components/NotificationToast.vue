<script setup lang="ts">
import { watch } from "vue";
import { storeToRefs } from "pinia";
import { useMessage } from "naive-ui";
import { useUiStore, type NotificationItem } from "@/stores/ui";

/**
 * NotificationToast — slim adapter over NaiveUI's `useMessage()`.
 *
 * The `ui` Pinia store remains the single source of truth for in-app
 * notifications (`pushNotification` / `dismissNotification`). This component
 * lives below `<NMessageProvider>` (mounted by `AppLayout.vue`) and forwards
 * each newly-pushed entry to the NaiveUI message API, then immediately
 * removes it from the store so the visual lifecycle is owned 100% by
 * NaiveUI. This eliminates the dual-render that would otherwise occur once
 * `useNotifications.notify()` writes to both the store and `useMessage()`.
 *
 * The component renders no markup of its own — it is a logic-only adapter.
 */
const ui = useUiStore();
const message = useMessage();
const { notifications } = storeToRefs(ui);

function dispatch(notif: NotificationItem) {
  switch (notif.level) {
    case "success":
      message.success(notif.message);
      break;
    case "warning":
      message.warning(notif.message);
      break;
    case "error":
      message.error(notif.message, { duration: 8000 });
      break;
    default:
      message.info(notif.message);
  }
  ui.dismissNotification(notif.id);
}

// Snapshot ids that have already been dispatched so we ignore the delete
// half of the watcher cycle (when `dismissNotification` mutates the array).
const dispatched = new Set<string>();

watch(
  notifications,
  (items) => {
    for (const n of items) {
      if (dispatched.has(n.id)) continue;
      dispatched.add(n.id);
      dispatch(n);
    }
  },
  { deep: true, immediate: true }
);
</script>

<template>
  <!-- Visual rendering is handled by <NMessageProvider> via useMessage();
       this hidden element exists only to satisfy the
       `vue/valid-template-root` rule and emits no styling. -->
  <span class="notification-toast-adapter" aria-hidden="true" hidden />
</template>
