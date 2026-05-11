import { describe, it, expect, beforeEach, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import StatusBar from "./StatusBar.vue";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

// StatusBar uses `useI18n()` (Task 7a NIT #6 — hardcoded strings →
// `t(...)` lookups), so the bare `mount()` no longer suffices. Use the
// shared helper that wires Pinia + i18n + the production router so the
// component can render under test.
function mountStatusBar() {
  return mountWithPlugins(StatusBar);
}

beforeEach(() => {
  vi.clearAllMocks();
  mockedInvoke.mockImplementation(async (command) => {
    if (command === "get_profile_info") return [];
    if (command === "get_permission_mode") return "Interactive";
    return undefined;
  });
});

describe("StatusBar", () => {
  it("calls get_permission_mode on mount", () => {
    mountStatusBar();
    expect(mockedInvoke).toHaveBeenCalledWith("get_permission_mode");
  });

  it("displays the permission mode in lowercase", async () => {
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "get_profile_info") return [];
      if (command === "get_permission_mode") return "Suggest";
      if (command === "list_mcp_servers") return [];
      return undefined;
    });
    const wrapper = mountStatusBar();
    await vi.waitFor(() => {
      expect(wrapper.text()).toContain("suggest");
    });
  });

  it("renders sessions count, streaming and connected status as text", async () => {
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "get_profile_info") return [];
      if (command === "get_permission_mode") return "Interactive";
      return undefined;
    });
    const wrapper = mountStatusBar();
    await flushPromises();

    const text = wrapper.text();
    expect(text).toContain("Sessions");
    expect(text).toContain("Streaming");
    expect(text).toContain("Connected");
  });
});
