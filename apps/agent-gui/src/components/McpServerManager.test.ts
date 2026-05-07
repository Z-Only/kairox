import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import McpServerManager from "./McpServerManager.vue";
import { mountWithPlugins } from "@/test-utils/mount";

// `McpServerManager.vue` calls `useI18n()`; bare `mount()` throws
// "Need to install with `app.use` function". `mountWithPlugins` installs
// i18n + router; `reusePinia: true` keeps the `beforeEach` pinia (and the
// store mutations driven through `useMcpStore()` in each test).
//
// Passing the extended-options shape returns `{ wrapper, router }`; we
// unwrap `.wrapper` so call-sites stay drop-in compatible with the prior
// `mount(...)` usage.
const mount = (comp: typeof McpServerManager) =>
  mountWithPlugins(comp, { reusePinia: true }).wrapper;

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import { useMcpStore } from "@/stores/mcp";

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("McpServerManager", () => {
  it("shows empty message when no servers configured", () => {
    const wrapper = mount(McpServerManager);
    expect(wrapper.text()).toContain("No MCP servers configured");
  });

  it("renders server list with names and statuses", () => {
    const mcp = useMcpStore();
    mcp.servers = [
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
    const mcp = useMcpStore();
    mcp.servers = [{ id: "github", status: "running", tool_count: 5 }];
    mcp.trustedServerIds = ["github"];
    const wrapper = mount(McpServerManager);
    expect(wrapper.text()).toContain("✅ Trusted");
  });

  it("shows untrusted warning for running but untrusted servers", () => {
    const mcp = useMcpStore();
    mcp.servers = [{ id: "github", status: "running", tool_count: 5 }];
    const wrapper = mount(McpServerManager);
    expect(wrapper.text()).toContain("⚠️ Not trusted");
  });

  it("shows Start button for stopped servers", () => {
    const mcp = useMcpStore();
    mcp.servers = [{ id: "slack", status: "stopped", tool_count: null }];
    const wrapper = mount(McpServerManager);
    const buttons = wrapper.findAll(".mcp-server-actions button");
    const buttonTexts = buttons.map((b) => b.text());
    expect(buttonTexts).toContain("Start");
  });

  it("shows Stop button for running servers", () => {
    const mcp = useMcpStore();
    mcp.servers = [{ id: "github", status: "running", tool_count: 5 }];
    const wrapper = mount(McpServerManager);
    const buttons = wrapper.findAll(".mcp-server-actions button");
    const buttonTexts = buttons.map((b) => b.text());
    expect(buttonTexts).toContain("Stop");
  });

  it("shows Restart button for failed servers", () => {
    const mcp = useMcpStore();
    mcp.servers = [{ id: "broken", status: "failed", tool_count: null }];
    const wrapper = mount(McpServerManager);
    const buttons = wrapper.findAll(".mcp-server-actions button");
    const buttonTexts = buttons.map((b) => b.text());
    expect(buttonTexts).toContain("Restart");
  });

  it("shows Trust button for untrusted running servers", () => {
    const mcp = useMcpStore();
    mcp.servers = [{ id: "github", status: "running", tool_count: 5 }];
    const wrapper = mount(McpServerManager);
    const buttons = wrapper.findAll(".mcp-server-actions button");
    const buttonTexts = buttons.map((b) => b.text());
    expect(buttonTexts).toContain("Trust");
  });

  it("shows Revoke button for trusted servers", () => {
    const mcp = useMcpStore();
    mcp.servers = [{ id: "github", status: "running", tool_count: 5 }];
    mcp.trustedServerIds = ["github"];
    const wrapper = mount(McpServerManager);
    const buttons = wrapper.findAll(".mcp-server-actions button");
    const buttonTexts = buttons.map((b) => b.text());
    expect(buttonTexts).toContain("Revoke");
  });

  it("shows tool count for running servers", () => {
    const mcp = useMcpStore();
    mcp.servers = [{ id: "github", status: "running", tool_count: 5 }];
    const wrapper = mount(McpServerManager);
    expect(wrapper.text()).toContain("5 tools");
  });

  it("shows error message for failed servers", () => {
    const mcp = useMcpStore();
    mcp.servers = [
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
    const mcp = useMcpStore();
    mockedInvoke.mockResolvedValueOnce(undefined); // start_mcp_server
    mockedInvoke.mockResolvedValueOnce([]); // list_mcp_servers (fetchServers)
    mcp.servers = [{ id: "slack", status: "stopped", tool_count: null }];
    const wrapper = mount(McpServerManager);
    await wrapper.findAll(".mcp-server-actions button")[0].trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("start_mcp_server", {
      serverId: "slack"
    });
  });
});
