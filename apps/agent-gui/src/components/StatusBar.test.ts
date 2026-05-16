import { describe, it, expect, beforeEach, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import StatusBar from "./StatusBar.vue";
import { mountWithPlugins } from "@/test-utils/mount";
import { useSessionStore } from "@/stores/session";

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
    return undefined;
  });
});

describe("StatusBar", () => {
  it("calls get_profile_info on mount", () => {
    mountStatusBar();
    expect(mockedInvoke).toHaveBeenCalledWith("get_profile_info");
  });

  it("displays the permission mode from the session store", async () => {
    const session = useSessionStore();
    session.permissionMode = "suggest";
    const wrapper = mountStatusBar();
    await flushPromises();
    expect(wrapper.text()).toContain("suggest");
  });

  it("renders sessions count, streaming and connected status as text", async () => {
    const wrapper = mountStatusBar();
    await flushPromises();

    const text = wrapper.text();
    expect(text).toContain("Sessions");
    expect(text).toContain("Streaming");
    expect(text).toContain("Connected");
  });
});
