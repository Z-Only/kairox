import { describe, it, expect, beforeEach, vi } from "vitest";
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
});

describe("StatusBar", () => {
  it("calls get_permission_mode on mount", () => {
    mockedInvoke.mockResolvedValueOnce("Suggest");
    // Also mock list_mcp_servers from fetchServers
    mockedInvoke.mockResolvedValueOnce([]);
    mountStatusBar();
    expect(mockedInvoke).toHaveBeenCalledWith("get_permission_mode");
  });

  it("displays the permission mode in lowercase", async () => {
    mockedInvoke.mockResolvedValueOnce("Suggest");
    mockedInvoke.mockResolvedValueOnce([]);
    const wrapper = mountStatusBar();
    await vi.waitFor(() => {
      expect(wrapper.text()).toContain("suggest");
    });
  });

  it("displays MCP status indicator", () => {
    mockedInvoke.mockResolvedValueOnce("Interactive");
    mockedInvoke.mockResolvedValueOnce([]);
    const wrapper = mountStatusBar();
    expect(wrapper.findComponent({ name: "McpStatusIndicator" }).exists()).toBe(true);
  });

  it("renders profile, sessions count, streaming and connected status as text", () => {
    mockedInvoke.mockResolvedValueOnce("Interactive");
    mockedInvoke.mockResolvedValueOnce([]);
    const wrapper = mountStatusBar();
    const text = wrapper.text();
    expect(text).toContain("profile:");
    expect(text).toContain("sessions:");
    expect(text).toContain("streaming: no");
    expect(text).toContain("connected: no");
  });
});
