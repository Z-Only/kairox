import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";

// Mock @tauri-apps/api/event so `listen()` always rejects.
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.reject(new Error("channel closed")))
}));

import { useTauriEvents } from "./useTauriEvents";
import { useUiStore } from "@/stores/ui";

const Dummy = defineComponent({
  setup() {
    useTauriEvents();
    return () => null;
  }
});

describe("useTauriEvents", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
  });

  it("surfaces a listen() rejection as an error notification", async () => {
    const wrapper = mount(Dummy);

    // Flush microtasks so the rejected unlistenPromise reaches the .catch handler.
    await Promise.resolve();
    await Promise.resolve();

    const ui = useUiStore();
    const errorNotice = ui.notifications.find(
      (n) =>
        n.level === "error" &&
        n.message.startsWith("Failed to subscribe to session events")
    );

    expect(errorNotice).toBeDefined();
    expect(errorNotice!.message).toContain("channel closed");

    wrapper.unmount();
  });
});
