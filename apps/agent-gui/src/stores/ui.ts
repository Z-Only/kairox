import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { usePreferredDark, useStorage } from "@vueuse/core";

export type NotificationLevel = "info" | "success" | "warning" | "error";
export interface NotificationItem {
  id: string;
  level: NotificationLevel;
  message: string;
  timestamp: number;
}
export type ThemeMode = "auto" | "light" | "dark";
export type SupportedLocale = "en" | "zh-CN";

export const useUiStore = defineStore("ui", () => {
  // ── Theme ───────────────────────────────────────────────
  // `colorMode` exposes the user's raw preference ("auto" | "light" | "dark"),
  // not the resolved system value. We avoid `useColorMode`/`useDark` directly
  // because both eagerly normalize "auto" to "light"/"dark" on first read,
  // which loses the user's intent and breaks round-tripping. `usePreferredDark`
  // reads the system media query without touching storage, so `isDark` derives
  // cleanly without overwriting the persisted preference.
  const colorMode = useStorage<ThemeMode>(
    "kairox.color-mode",
    "auto",
    undefined,
    {
      flush: "sync",
      serializer: {
        read: (v) =>
          v === "auto" || v === "light" || v === "dark"
            ? (v as ThemeMode)
            : "auto",
        write: (v) => v
      }
    }
  );
  const preferredDark = usePreferredDark();
  const isDark = computed(() =>
    colorMode.value === "auto"
      ? preferredDark.value
      : colorMode.value === "dark"
  );

  function setTheme(mode: ThemeMode) {
    colorMode.value = mode;
  }

  // ── Locale ──────────────────────────────────────────────
  // `flush: "sync"` so that `setLocale(...)` reflects in localStorage in the
  // same tick — the surrounding test suite asserts persistence synchronously.
  const locale = useStorage<SupportedLocale>("kairox.locale", "en", undefined, {
    flush: "sync",
    serializer: {
      read: (v) => (v === "zh-CN" || v === "en" ? v : "en"),
      write: (v) => v
    }
  });

  function setLocale(next: SupportedLocale) {
    locale.value = next;
  }

  // ── Sidebar (future-proof) ──────────────────────────────
  const sidebarCollapsed = useStorage(
    "kairox.sidebar-collapsed",
    false,
    undefined,
    {
      flush: "sync"
    }
  );

  // ── Notifications ───────────────────────────────────────
  const notifications = ref<NotificationItem[]>([]);

  function pushNotification(level: NotificationLevel, message: string) {
    notifications.value.push({
      id: crypto.randomUUID(),
      level,
      message,
      timestamp: Date.now()
    });
  }

  function dismissNotification(id: string) {
    notifications.value = notifications.value.filter((n) => n.id !== id);
  }

  return {
    colorMode,
    isDark,
    setTheme,
    locale,
    setLocale,
    sidebarCollapsed,
    notifications,
    pushNotification,
    dismissNotification
  };
});
