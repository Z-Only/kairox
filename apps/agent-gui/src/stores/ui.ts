// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Pinia stores are plain `.ts` modules and
// must import `defineStore`, `ref`, `computed`, and the whitelisted
// `@vueuse/core` hooks explicitly — otherwise Vite ESM evaluates this
// module before any auto-import shim could exist and the browser hits
// `ReferenceError: defineStore is not defined`.
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { useStorage, usePreferredDark } from "@vueuse/core";

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
  const colorMode = useStorage<ThemeMode>("kairox.color-mode", "auto", undefined, {
    flush: "sync",
    serializer: {
      read: (v) => (v === "auto" || v === "light" || v === "dark" ? (v as ThemeMode) : "auto"),
      write: (v) => v
    }
  });
  const preferredDark = usePreferredDark();
  const isDark = computed(() =>
    colorMode.value === "auto" ? preferredDark.value : colorMode.value === "dark"
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
  const sidebarCollapsed = useStorage("kairox.sidebar-collapsed", false, undefined, {
    flush: "sync"
  });

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
    // INVARIANT: this MUST replace `notifications.value` with a fresh array
    // (filter), never mutate the existing array in place (no `splice`, no
    // `pop`). NotificationToast.vue's deep watcher iterates `notifications`
    // with `for (const n of items)` and synchronously calls
    // `ui.dismissNotification(n.id)` on every iteration. An in-place
    // `splice(idx, 1)` would shift the remaining elements down and the
    // for-of iterator would skip the next entry, silently dropping
    // notifications. Reassigning to a filtered copy keeps the iterator's
    // snapshot stable and is covered by the immutability regression test in
    // ui.test.ts.
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
