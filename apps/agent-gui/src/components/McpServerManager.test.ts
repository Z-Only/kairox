import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount } from "@vue/test-utils";
import McpServerManager from "./McpServerManager.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));
vi.mock("../composables/useNotifications", () => ({
  addNotification: vi.fn(),
  dismissNotification: vi.fn(),
  notifications: []
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import { mcpState } from "../stores/mcp";

beforeEach(() => {
  mcpState.servers = [];
  mcpState.trustedServerIds = [];
  mcpState.loading = false;
  vi.clearAllMocks();
});

describe("McpServerManager", () => {
  it("shows empty message when no servers configured", () => {
    const wrapper = mount(McpServerManager);
    expect(wrapper.text()).toContain("No MCP servers configured");
  });

  it("renders server list with names and statuses", () => {
    mcpState.servers = [
      { id: "github", status: "running", tool_count: 5 },
      { id: "slack", status: "stopped", tool_count: null }
    ];
    const wrapper = mount(McpServerManager);
    expect(wrapper.text()).toContain("github");
    expect(wrapper.text()).toContain("🟢 Running");
    expect(wrapper.text()).toContain("slack");
    expect(wrapper.text()).toContain("⚪ Stopped");
  });

  it("shows trust badge for trusted servers", () => {
    mcpState.servers = [{ id: "github", status: "running", tool_count: 5 }];
    mcpState.trustedServerIds = ["github"];
    const wrapper = mount(McpServerManager);
    expect(wrapper.text()).toContain("✅ Trusted");
  });

  it("shows untrusted warning for running but untrusted servers", () => {
    mcpState.servers = [{ id: "github", status: "running", tool_count: 5 }];
    const wrapper = mount(McpServerManager);
    expect(wrapper.text()).toContain("⚠️ Not trusted");
  });

  it("shows Start button for stopped servers", () => {
    mcpState.servers = [{ id: "slack", status: "stopped", tool_count: null }];
    const wrapper = mount(McpServerManager);
    const buttons = wrapper.findAll(".mcp-server-actions button");
    const buttonTexts = buttons.map((b) => b.text());
    expect(buttonTexts).toContain("Start");
  });

  it("shows Stop button for running servers", () => {
    mcpState.servers = [{ id: "github", status: "running", tool_count: 5 }];
    const wrapper = mount(McpServerManager);
    const buttons = wrapper.findAll(".mcp-server-actions button");
    const buttonTexts = buttons.map((b) => b.text());
    expect(buttonTexts).toContain("Stop");
  });

  it("shows Restart button for failed servers", () => {
    mcpState.servers = [{ id: "broken", status: "failed", tool_count: null }];
    const wrapper = mount(McpServerManager);
    const buttons = wrapper.findAll(".mcp-server-actions button");
    const buttonTexts = buttons.map((b) => b.text());
    expect(buttonTexts).toContain("Restart");
  });

  it("shows Trust button for untrusted running servers", () => {
    mcpState.servers = [{ id: "github", status: "running", tool_count: 5 }];
    const wrapper = mount(McpServerManager);
    const buttons = wrapper.findAll(".mcp-server-actions button");
    const buttonTexts = buttons.map((b) => b.text());
    expect(buttonTexts).toContain("Trust");
  });

  it("shows Revoke button for trusted servers", () => {
    mcpState.servers = [{ id: "github", status: "running", tool_count: 5 }];
    mcpState.trustedServerIds = ["github"];
    const wrapper = mount(McpServerManager);
    const buttons = wrapper.findAll(".mcp-server-actions button");
    const buttonTexts = buttons.map((b) => b.text());
    expect(buttonTexts).toContain("Revoke");
  });

  it("shows tool count for running servers", () => {
    mcpState.servers = [{ id: "github", status: "running", tool_count: 5 }];
    const wrapper = mount(McpServerManager);
    expect(wrapper.text()).toContain("5 tools");
  });

  it("shows error message for failed servers", () => {
    mcpState.servers = [
      {
        id: "broken",
        status: "failed",
        tool_count: null,
        error: "connection refused"
      }
    ];
    const wrapper = mount(McpServerManager);
    expect(wrapper.text()).toContain("connection refused");
  });

  it("emits close on close button click", async () => {
    const wrapper = mount(McpServerManager);
    await wrapper.find(".mcp-close-btn").trigger("click");
    expect(wrapper.emitted("close")).toHaveLength(1);
  });

  it("invokes start_mcp_server on Start click", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined); // start_mcp_server
    mockedInvoke.mockResolvedValueOnce([]); // list_mcp_servers (fetchServers)
    mcpState.servers = [{ id: "slack", status: "stopped", tool_count: null }];
    const wrapper = mount(McpServerManager);
    await wrapper.findAll(".mcp-server-actions button")[0].trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("start_mcp_server", {
      serverId: "slack"
    });
  });
});
