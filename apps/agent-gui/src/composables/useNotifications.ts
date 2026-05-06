import { storeToRefs } from "pinia";
import { useUiStore, type NotificationLevel } from "@/stores/ui";

export type Notification = {
  id: string;
  type: NotificationLevel;
  message: string;
  timestamp: number;
};

export function useNotifications() {
  const ui = useUiStore();
  const { notifications } = storeToRefs(ui);
  return {
    notifications,
    addNotification: (type: NotificationLevel, message: string) =>
      ui.pushNotification(type, message),
    dismissNotification: (id: string) => ui.dismissNotification(id)
  };
}

// Back-compat top-level fn used by other modules (App.vue, session store, etc.)
export function addNotification(
  type: NotificationLevel,
  message: string
): void {
  useUiStore().pushNotification(type, message);
}

export function dismissNotification(id: string): void {
  useUiStore().dismissNotification(id);
}
