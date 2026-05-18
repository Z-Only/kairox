import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useUiStore } from "@/stores/ui";

describe("ui store", () => {
  beforeEach(() => {
    window.localStorage.clear();
    setActivePinia(createPinia());
  });

  describe("notifications", () => {
    it("starts empty", () => {
      const ui = useUiStore();
      expect(ui.notifications).toEqual([]);
    });

    it("push then dismiss", () => {
      const ui = useUiStore();
      ui.pushNotification("info", "hello");
      expect(ui.notifications.length).toBe(1);
      const id = ui.notifications[0].id;
      ui.dismissNotification(id);
      expect(ui.notifications).toEqual([]);
    });

    it("each notification has unique id", () => {
      const ui = useUiStore();
      ui.pushNotification("info", "a");
      ui.pushNotification("info", "b");
      const ids = ui.notifications.map((n) => n.id);
      expect(new Set(ids).size).toBe(2);
    });

    it("dismissNotification replaces the array reference (immutability contract)", () => {
      // NotificationToast.vue's watcher iterates `notifications` with
      // for-of while synchronously dismissing each entry. If
      // dismissNotification mutated the array in place (splice), the
      // iterator would skip the next element and silently drop
      // notifications. This test locks the contract by asserting that
      // dismissNotification produces a brand-new array reference rather
      // than mutating the original in place — see the INVARIANT comment
      // in stores/ui.ts.
      const ui = useUiStore();
      ui.pushNotification("info", "a");
      ui.pushNotification("info", "b");
      const originalRef = ui.notifications;
      const targetId = ui.notifications[0].id;
      ui.dismissNotification(targetId);
      expect(ui.notifications).not.toBe(originalRef);
      expect(ui.notifications.length).toBe(1);
      expect(ui.notifications[0].id).not.toBe(targetId);
    });
  });

  describe("theme", () => {
    it("defaults to auto color mode", () => {
      const ui = useUiStore();
      expect(ui.colorMode).toBe("auto");
    });

    it("setTheme updates the colorMode ref", () => {
      const ui = useUiStore();
      ui.setTheme("dark");
      expect(ui.colorMode).toBe("dark");
    });
  });

  describe("locale", () => {
    it("defaults to system when storage is empty", () => {
      const ui = useUiStore();
      expect(ui.locale).toBe("system");
    });

    it("setLocale persists to localStorage", () => {
      const ui = useUiStore();
      ui.setLocale("zh-CN");
      expect(ui.locale).toBe("zh-CN");
      expect(window.localStorage.getItem("kairox.locale")).toBe("zh-CN");
    });

    it("rejects invalid locale from storage", () => {
      window.localStorage.setItem("kairox.locale", "fr-FR");
      setActivePinia(createPinia());
      const ui = useUiStore();
      expect(ui.locale).toBe("system");
    });
  });

  describe("workbench sidebars", () => {
    it("persists independent collapse state for the left and right sidebars", () => {
      const ui = useUiStore();

      ui.setSidebarCollapsed("left", true);
      ui.setSidebarCollapsed("right", true);

      expect(ui.leftSidebarCollapsed).toBe(true);
      expect(ui.rightSidebarCollapsed).toBe(true);
      expect(window.localStorage.getItem("kairox.left-sidebar-collapsed")).toBe("true");
      expect(window.localStorage.getItem("kairox.right-sidebar-collapsed")).toBe("true");
    });

    it("persists clamped sidebar widths", () => {
      const ui = useUiStore();

      ui.setSidebarWidth("left", 50);
      ui.setSidebarWidth("right", 999);

      expect(ui.leftSidebarWidth).toBe(180);
      expect(ui.rightSidebarWidth).toBe(420);
      expect(window.localStorage.getItem("kairox.left-sidebar-width")).toBe("180");
      expect(window.localStorage.getItem("kairox.right-sidebar-width")).toBe("420");
    });
  });
});
