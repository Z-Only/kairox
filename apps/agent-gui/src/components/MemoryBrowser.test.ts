import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import MemoryBrowser from "./MemoryBrowser.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));
vi.mock("../composables/useNotifications", () => ({
  addNotification: vi.fn(),
  dismissNotification: vi.fn(),
  notifications: []
}));
vi.mock("../stores/session", () => ({
  sessionState: {
    currentSessionId: "ses_1"
  }
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import { memoryState } from "../stores/memory";

beforeEach(() => {
  memoryState.memories = [];
  memoryState.loading = false;
  memoryState.filter = "all";
  memoryState.searchQuery = "";
  vi.clearAllMocks();
  mockedInvoke.mockResolvedValueOnce([]);
});

describe("MemoryBrowser", () => {
  it("shows empty state when no memories", () => {
    const wrapper = mount(MemoryBrowser);
    expect(wrapper.text()).toContain("No memories");
  });

  it("shows loading state", () => {
    memoryState.loading = true;
    const wrapper = mount(MemoryBrowser);
    expect(wrapper.text()).toContain("Loading");
  });

  it("renders memory items with scope info", async () => {
    memoryState.memories = [
      { id: "m1", scope: "user", key: "lang", content: "Rust", accepted: true },
      {
        id: "m2",
        scope: "session",
        key: null,
        content: "Temp note",
        accepted: true
      }
    ];
    const wrapper = mount(MemoryBrowser);
    expect(wrapper.text()).toContain("Rust");
    expect(wrapper.text()).toContain("Temp note");
    expect(wrapper.text()).toContain("user");
  });

  it("changes active scope filter on click", async () => {
    mockedInvoke.mockResolvedValueOnce([]);
    const wrapper = mount(MemoryBrowser);
    const buttons = wrapper.findAll(".scope-btn");
    const userBtn = buttons.find((b) => b.text() === "User");
    if (userBtn) {
      await userBtn.trigger("click");
      expect(userBtn.classes()).toContain("active");
    }
  });
});
