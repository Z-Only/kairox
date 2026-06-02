// `unplugin-auto-import` only injects globals into `.vue` SFCs (we keep
// `dirs: []` per spec §3 Q7). Pinia stores are plain `.ts` modules and
// must import `defineStore`, `ref`, `computed`, and the whitelisted
// `@vueuse/core` hooks explicitly — otherwise Vite ESM evaluates this
// module before any auto-import shim could exist and the browser hits
// `ReferenceError: defineStore is not defined`.
import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { useStorage, usePreferredDark } from "@vueuse/core";
import { commands, type GuiSettingsView } from "@/generated/commands";
import {
  LOCALE_STORAGE_KEY,
  markLocalePreferenceExplicit,
  normalizeStoredLocale,
  readStoredLocalePreference,
  type SupportedLocale
} from "@/locales/localePreference";

export type { SupportedLocale } from "@/locales/localePreference";

export type NotificationLevel = "info" | "success" | "warning" | "error";
export interface NotificationItem {
  id: string;
  level: NotificationLevel;
  message: string;
  timestamp: number;
}
export type ThemeMode = "auto" | "light" | "dark";
export type WorkbenchSidebarSide = "left" | "right";

export interface ToastItem {
  id: string;
  message: string;
  type: NotificationLevel;
  duration: number;
}

type CommandResult<T> = { status: "ok"; data: T } | { status: "error"; error: string };

function isCommandResult<T>(value: unknown): value is CommandResult<T> {
  return (
    typeof value === "object" &&
    value !== null &&
    "status" in value &&
    ((value as { status: unknown }).status === "ok" ||
      (value as { status: unknown }).status === "error")
  );
}

async function unwrapCommandResult<T>(resultPromise: Promise<T | CommandResult<T>>): Promise<T> {
  const result = await resultPromise;
  if (!isCommandResult<T>(result)) return result;
  if (result.status === "ok") return result.data;
  throw new Error(result.error);
}

function formatError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export const useUiStore = defineStore("ui", () => {
  const SIDEBAR_MIN_WIDTH = 180;
  const SIDEBAR_MAX_WIDTH = 420;

  function clampSidebarWidth(value: number): number {
    if (!Number.isFinite(value)) return SIDEBAR_MIN_WIDTH;
    return Math.min(SIDEBAR_MAX_WIDTH, Math.max(SIDEBAR_MIN_WIDTH, Math.round(value)));
  }

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
  const locale = useStorage<SupportedLocale>(
    LOCALE_STORAGE_KEY,
    readStoredLocalePreference(),
    undefined,
    {
      flush: "sync",
      serializer: {
        read: normalizeStoredLocale,
        write: (v) => v
      }
    }
  );

  function setLocale(next: SupportedLocale) {
    markLocalePreferenceExplicit();
    locale.value = next;
  }

  // ── GUI settings ───────────────────────────────────────
  const devtoolsEnabled = ref(false);
  const devtoolsDefaultEnabled = ref(false);
  const devtoolsRequiresRestart = ref(false);
  const guiSettingsLoading = ref(false);
  const guiSettingsError = ref<string | null>(null);

  function applyGuiSettings(settings: GuiSettingsView): void {
    devtoolsEnabled.value = settings.devtools_enabled;
    devtoolsDefaultEnabled.value = settings.default_devtools_enabled;
    devtoolsRequiresRestart.value = settings.requires_restart;
  }

  async function loadGuiSettings(): Promise<void> {
    guiSettingsLoading.value = true;
    guiSettingsError.value = null;
    try {
      applyGuiSettings(await unwrapCommandResult(commands.getGuiSettings()));
    } catch (error) {
      guiSettingsError.value = formatError(error);
    } finally {
      guiSettingsLoading.value = false;
    }
  }

  async function setDevtoolsEnabled(enabled: boolean): Promise<void> {
    const previous = devtoolsEnabled.value;
    devtoolsEnabled.value = enabled;
    guiSettingsError.value = null;
    try {
      applyGuiSettings(await unwrapCommandResult(commands.setGuiDevtoolsEnabled(enabled)));
    } catch (error) {
      devtoolsEnabled.value = previous;
      guiSettingsError.value = formatError(error);
      throw error;
    }
  }

  // ── Sidebar (future-proof) ──────────────────────────────
  const sidebarCollapsed = useStorage("kairox.sidebar-collapsed", false, undefined, {
    flush: "sync"
  });
  const leftSidebarCollapsed = useStorage("kairox.left-sidebar-collapsed", false, undefined, {
    flush: "sync"
  });
  const rightSidebarCollapsed = useStorage("kairox.right-sidebar-collapsed", false, undefined, {
    flush: "sync"
  });
  const leftSidebarWidth = useStorage("kairox.left-sidebar-width", 220, undefined, {
    flush: "sync",
    serializer: {
      read: (v) => clampSidebarWidth(Number(v ?? 220)),
      write: (v) => String(clampSidebarWidth(v))
    }
  });
  const rightSidebarWidth = useStorage("kairox.right-sidebar-width", 280, undefined, {
    flush: "sync",
    serializer: {
      read: (v) => clampSidebarWidth(Number(v ?? 280)),
      write: (v) => String(clampSidebarWidth(v))
    }
  });

  function setSidebarCollapsed(side: WorkbenchSidebarSide, collapsed: boolean) {
    if (side === "left") {
      leftSidebarCollapsed.value = collapsed;
      sidebarCollapsed.value = collapsed;
      return;
    }
    rightSidebarCollapsed.value = collapsed;
  }

  function toggleSidebar(side: WorkbenchSidebarSide) {
    if (side === "left") {
      setSidebarCollapsed("left", !leftSidebarCollapsed.value);
      return;
    }
    setSidebarCollapsed("right", !rightSidebarCollapsed.value);
  }

  function setSidebarWidth(side: WorkbenchSidebarSide, width: number) {
    if (side === "left") {
      leftSidebarWidth.value = clampSidebarWidth(width);
      return;
    }
    rightSidebarWidth.value = clampSidebarWidth(width);
  }

  // ── Notifications ───────────────────────────────────────
  const notifications = ref<NotificationItem[]>([]);

  function pushNotification(level: NotificationLevel, message: string) {
    notifications.value.push({
      id: crypto.randomUUID(),
      level,
      message,
      timestamp: Date.now()
    });
    // Bridge to toast system so every notification also produces a visual toast.
    addToast(message, level);
  }

  function dismissNotification(id: string) {
    notifications.value = notifications.value.filter((n) => n.id !== id);
  }

  // ── Toasts (visual notifications) ──
  const toasts = ref<ToastItem[]>([]);
  let toastCounter = 0;

  function addToast(message: string, type: NotificationLevel = "info", duration = 4000): string {
    const id = `toast-${++toastCounter}-${Date.now()}`;
    toasts.value = [...toasts.value, { id, message, type, duration }];
    return id;
  }

  function removeToast(id: string) {
    toasts.value = toasts.value.filter((t) => t.id !== id);
  }

  return {
    colorMode,
    isDark,
    setTheme,
    locale,
    setLocale,
    devtoolsEnabled,
    devtoolsDefaultEnabled,
    devtoolsRequiresRestart,
    guiSettingsLoading,
    guiSettingsError,
    loadGuiSettings,
    setDevtoolsEnabled,
    sidebarCollapsed,
    leftSidebarCollapsed,
    rightSidebarCollapsed,
    leftSidebarWidth,
    rightSidebarWidth,
    setSidebarCollapsed,
    toggleSidebar,
    setSidebarWidth,
    notifications,
    pushNotification,
    dismissNotification,
    toasts,
    addToast,
    removeToast
  };
});
