import { reactive } from "vue";

export interface Notification {
  id: string;
  type: "error" | "warning" | "info";
  message: string;
  timestamp: number;
}

export const notifications = reactive<Notification[]>([]);

let nextId = 0;

export function addNotification(
  type: Notification["type"],
  message: string
): void {
  const id = `notif-${nextId++}`;
  notifications.push({ id, type, message, timestamp: Date.now() });
  // Auto-dismiss after 8 seconds
  setTimeout(() => dismissNotification(id), 8000);
}

export function dismissNotification(id: string): void {
  const idx = notifications.findIndex((n) => n.id === id);
  if (idx !== -1) notifications.splice(idx, 1);
}
