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
  // Drop the dispatched id from the Set after the message handler has been
  // invoked. See the JSDoc on `dispatched` below for the leak rationale.
  dispatched.delete(notif.id);
  ui.dismissNotification(notif.id);
}

/**
 * Tracks ids that are mid-dispatch within the current watcher tick so that
 * the second half of the same tick — when `ui.dismissNotification` shrinks
 * `notifications.value` and the deep watcher re-fires — does not see the
 * still-present entry as "new" and forward it twice.
 *
 * Notification ids come from `crypto.randomUUID()` (see `ui.pushNotification`)
 * so they never repeat across the lifetime of the app. Without the
 * `dispatched.delete(id)` call inside `dispatch()`, this Set would grow
 * monotonically for the entire lifetime of `<AppLayout>` (i.e. forever) —
 * a slow but real leak in long-running sessions. We delete the id after the
 * handler returns instead of keeping a permanent history because the only
 * job of this Set is intra-tick deduplication.
 */
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
