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

import { useSessionStore } from "@/stores/session";
import { useI18n } from "vue-i18n";

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
    if (command === "list_mcp_servers") return [];
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

  it("displays MCP status indicator", () => {
    const wrapper = mountStatusBar();
    expect(wrapper.findComponent({ name: "McpStatusIndicator" }).exists()).toBe(true);
  });

  it("renders provider/model, sessions count, streaming and connected status as text", async () => {
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "get_profile_info") {
        return [
          {
            alias: "deep",
            provider: "anthropic",
            model_id: "claude-3-5-sonnet",
            local: false,
            has_api_key: true
          }
        ];
      }
      if (command === "get_permission_mode") return "Interactive";
      if (command === "list_mcp_servers") return [];
      return undefined;
    });
    const wrapper = mountStatusBar();
    const session = useSessionStore();
    session.currentProfile = "deep";
    session.profileInfos = [
      {
        alias: "deep",
        provider: "anthropic",
        model_id: "claude-3-5-sonnet",
        local: false,
        has_api_key: true
      }
    ] as never;
    await flushPromises();

    const text = wrapper.text();
    // activeProfileDisplay formats as "Provider · Model" (e.g., "Anthropic · Claude 3.5 Sonnet")
    expect(text).toContain("Anthropic");
    expect(text).toContain("Claude 3.5 Sonnet");
    // Check for i18n-translated labels (defaults to English in tests)
    expect(text).toContain("Sessions");
    expect(text).toContain("Streaming");
    expect(text).toContain("Connected");
  });
});
