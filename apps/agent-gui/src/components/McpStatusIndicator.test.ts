import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import McpStatusIndicator from "./McpStatusIndicator.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { useMcpStore } from "@/stores/mcp";

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("McpStatusIndicator", () => {
  it("shows MCP label with no servers", () => {
    const wrapper = mount(McpStatusIndicator);
    expect(wrapper.text()).toContain("MCP");
    expect(wrapper.find(".mcp-status").classes()).toContain("mcp-none");
  });

  it("shows stopped state when servers exist but none running", () => {
    const mcp = useMcpStore();
    mcp.servers = [{ id: "s1", status: "stopped", tool_count: null }];
    const wrapper = mount(McpStatusIndicator);
    expect(wrapper.text()).toContain("0 MCP");
    expect(wrapper.find(".mcp-status").classes()).toContain("mcp-stopped");
  });

  it("shows running state with green dot", () => {
    const mcp = useMcpStore();
    mcp.servers = [
      { id: "s1", status: "running", tool_count: 3 },
      { id: "s2", status: "stopped", tool_count: null }
    ];
    const wrapper = mount(McpStatusIndicator);
    expect(wrapper.text()).toContain("1 MCP");
    expect(wrapper.find(".mcp-status").classes()).toContain("mcp-running");
    expect(wrapper.text()).toContain("🟢");
  });

  it("shows failed state with red dot", () => {
    const mcp = useMcpStore();
    mcp.servers = [{ id: "s1", status: "failed", tool_count: null }];
    const wrapper = mount(McpStatusIndicator);
    expect(wrapper.find(".mcp-status").classes()).toContain("mcp-failed");
    expect(wrapper.text()).toContain("🔴");
  });

  it("prioritizes failed over running", () => {
    const mcp = useMcpStore();
    mcp.servers = [
      { id: "s1", status: "running", tool_count: 3 },
      { id: "s2", status: "failed", tool_count: null }
    ];
    const wrapper = mount(McpStatusIndicator);
    expect(wrapper.find(".mcp-status").classes()).toContain("mcp-failed");
  });

  it("emits click event", async () => {
    const wrapper = mount(McpStatusIndicator);
    await wrapper.find(".mcp-status").trigger("click");
    expect(wrapper.emitted("click")).toHaveLength(1);
  });

  it("audit anchors: exposes stable MCP status pilot selector", () => {
    const wrapper = mount(McpStatusIndicator);

    expect(wrapper.find('[data-test="mcp-status-indicator"]').exists()).toBe(true);
  });
});
