import { vi } from "vitest";
import type { NotificationItem } from "@/stores/ui";

/**
 * Canonical mock factory for `useUiStore()` consumed by store unit tests.
 *
 * **SHAPE-ONLY mock.** Reactive fields (`colorMode`, `isDark`, `locale`,
 * `sidebarCollapsed`) are plain values — writes to them do NOT propagate to
 * watchers. Only suitable for tests that spy on actions (`pushNotification`,
 * etc.). For reactivity-dependent tests, use `setActivePinia(createPinia())`
 * and the real store.
 *
 * Returns the full shape of the real ui store (notifications + theme + locale
 * + sidebar) so that no test ever silently destructures `undefined` when a new
 * store action starts reading from `useUiStore()`. Tests that need to assert
 * on `pushNotification` calls should pass their own spy via
 * `overrides.pushNotification` so the spy reference stays accessible.
 */
export interface UiStoreMockOverrides {
  pushNotification?: ReturnType<typeof vi.fn>;
  dismissNotification?: ReturnType<typeof vi.fn>;
}

export function createUiStoreMock(overrides: UiStoreMockOverrides = {}) {
  return {
    // notifications
    notifications: [] as NotificationItem[],
    pushNotification: overrides.pushNotification ?? vi.fn(),
    dismissNotification: overrides.dismissNotification ?? vi.fn(),
    // theme
    colorMode: "auto" as const,
    isDark: false,
    setTheme: vi.fn(),
    // locale
    locale: "en" as const,
    setLocale: vi.fn(),
    // sidebar
    sidebarCollapsed: false
  };
}
