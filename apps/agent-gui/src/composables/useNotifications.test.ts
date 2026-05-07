import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useNotifications } from "./useNotifications";
import { useUiStore } from "@/stores/ui";

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("useNotifications", () => {
  it("notify() writes to the ui store", () => {
    const { notify } = useNotifications();
    const ui = useUiStore();
    notify("info", "hello");
    expect(ui.notifications).toHaveLength(1);
    expect(ui.notifications[0].level).toBe("info");
    expect(ui.notifications[0].message).toBe("hello");
  });

  it("notify() also triggers a toast via addToast", () => {
    const { notify } = useNotifications();
    const ui = useUiStore();
    notify("error", "boom");
    expect(ui.toasts).toHaveLength(1);
    expect(ui.toasts[0].type).toBe("error");
    expect(ui.toasts[0].message).toBe("boom");
  });
});
