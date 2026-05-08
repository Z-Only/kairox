import { useUiStore, type NotificationLevel } from "@/stores/ui";

/**
 * Convenience wrapper around the ui store's notification API.
 * Writes to the store which triggers the ToastContainer visual layer.
 */
export function useNotifications() {
  const ui = useUiStore();

  function notify(level: NotificationLevel, content: string) {
    ui.pushNotification(level, content);
  }

  return { notify };
}
