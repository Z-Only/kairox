import { describe, it, expect, beforeEach, afterEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useUiStore } from "@/stores/ui";
import {
  addNotification,
  dismissNotification
} from "@/composables/useNotifications";

describe("useNotifications", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("addNotification pushes to array", () => {
    const ui = useUiStore();
    addNotification("info", "hello world");
    expect(ui.notifications).toHaveLength(1);
    expect(ui.notifications[0].level).toBe("info");
    expect(ui.notifications[0].message).toBe("hello world");
  });

  it("addNotification auto-increments IDs", () => {
    const ui = useUiStore();
    addNotification("info", "first");
    addNotification("error", "second");
    expect(ui.notifications).toHaveLength(2);
    expect(ui.notifications[0].id).not.toBe(ui.notifications[1].id);
  });

  it("dismissNotification removes by id", () => {
    const ui = useUiStore();
    addNotification("info", "removeme");
    const id = ui.notifications[0].id;
    dismissNotification(id);
    expect(ui.notifications).toHaveLength(0);
  });

  it("dismissNotification no crash on missing id", () => {
    const ui = useUiStore();
    addNotification("info", "stay");
    dismissNotification("nonexistent-id");
    expect(ui.notifications).toHaveLength(1);
  });

  it("type differentiation", () => {
    const ui = useUiStore();
    addNotification("error", "err");
    addNotification("warning", "warn");
    addNotification("info", "inf");
    expect(ui.notifications).toHaveLength(3);
    expect(ui.notifications[0].level).toBe("error");
    expect(ui.notifications[1].level).toBe("warning");
    expect(ui.notifications[2].level).toBe("info");
  });
});
