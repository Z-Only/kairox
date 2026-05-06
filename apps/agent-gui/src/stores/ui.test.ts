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
    it("defaults to en when storage is empty", () => {
      const ui = useUiStore();
      expect(ui.locale).toBe("en");
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
      expect(ui.locale).toBe("en");
    });
  });
});
