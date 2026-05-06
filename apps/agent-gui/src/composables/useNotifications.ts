import { useMessage } from "naive-ui";
import { useUiStore, type NotificationLevel } from "@/stores/ui";

/**
 * Bridges the ui store (source of truth for in-app notifications) with
 * NaiveUI's transient `useMessage()` toasts. Must be called from inside a
 * component that lives below `<NMessageProvider>` (i.e. anywhere under
 * `AppLayout.vue`).
 *
 * The ui store keeps the persistent notification log (`notifications` ref +
 * `pushNotification` action) so existing consumers — including tests that
 * spy on `pushNotification` — keep working unchanged. The transient toast is
 * a presentation layer on top.
 */
export function useNotifications() {
  const ui = useUiStore();
  const message = useMessage();

  function notify(level: NotificationLevel, content: string) {
    ui.pushNotification(level, content);
    switch (level) {
      case "success":
        message.success(content);
        break;
      case "warning":
        message.warning(content);
        break;
      case "error":
        message.error(content);
        break;
      default:
        message.info(content);
    }
  }

  return { notify };
}
