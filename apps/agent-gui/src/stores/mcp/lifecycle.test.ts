import { describe, it, expect, beforeEach, vi } from "vitest";
import { createMcpState } from "./state";
import { createLifecycle } from "./lifecycle";
import { createUiStoreMock } from "@/test-utils/uiMock";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

const pushNotificationSpy = vi.fn();
vi.mock("@/stores/ui", () => ({
  useUiStore: () => createUiStoreMock({ pushNotification: pushNotificationSpy })
}));

import { invoke } from "@tauri-apps/api/core";

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  vi.clearAllMocks();
});

function setup() {
  const state = createMcpState();
  const updateToolCount = vi.fn();
  const loadDisabledTools = vi.fn().mockResolvedValue(undefined);
  const actions = createLifecycle(state, { updateToolCount, loadDisabledTools });
  return { state, actions, updateToolCount, loadDisabledTools };
}

describe("fetchServers", () => {
  it("populates servers from invoke result", async () => {
    const { state, actions } = setup();
    mockedInvoke.mockResolvedValueOnce([
      { id: "s1", status: "running", tool_count: 3 },
      { id: "s2", status: "stopped", tool_count: null }
    ]);

    await actions.fetchServers();

    expect(mockedInvoke).toHaveBeenCalledWith("list_mcp_servers");
    expect(state.servers.value).toHaveLength(2);
    expect(state.loading.value).toBe(false);
  });

  it("notifies on error", async () => {
    const { state, actions } = setup();
    mockedInvoke.mockRejectedValueOnce(new Error("network error"));

    await actions.fetchServers();

    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("network error")
    );
    expect(state.loading.value).toBe(false);
  });
});

describe("startServer", () => {
  it("invokes start_mcp_server and refreshes list", async () => {
    const { actions } = setup();
    mockedInvoke.mockResolvedValueOnce(undefined);
    mockedInvoke.mockResolvedValueOnce([]);

    await actions.startServer("s1");

    expect(mockedInvoke).toHaveBeenCalledWith("start_mcp_server", { serverId: "s1" });
    expect(mockedInvoke).toHaveBeenCalledWith("list_mcp_servers");
  });

  it("notifies on error", async () => {
    const { actions } = setup();
    mockedInvoke.mockRejectedValueOnce(new Error("start failed"));

    await actions.startServer("s1");

    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("start failed")
    );
  });
});

describe("stopServer", () => {
  it("invokes stop_mcp_server and refreshes list", async () => {
    const { actions } = setup();
    mockedInvoke.mockResolvedValueOnce(undefined);
    mockedInvoke.mockResolvedValueOnce([]);

    await actions.stopServer("s1");

    expect(mockedInvoke).toHaveBeenCalledWith("stop_mcp_server", { serverId: "s1" });
    expect(mockedInvoke).toHaveBeenCalledWith("list_mcp_servers");
  });

  it("notifies on error", async () => {
    const { actions } = setup();
    mockedInvoke.mockRejectedValueOnce(new Error("stop failed"));

    await actions.stopServer("s1");

    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("stop failed")
    );
  });
});

describe("trustServer", () => {
  it("invokes trust_mcp_server and adds to trusted list", async () => {
    const { state, actions } = setup();
    mockedInvoke.mockResolvedValueOnce(undefined);

    await actions.trustServer("s1");

    expect(mockedInvoke).toHaveBeenCalledWith("trust_mcp_server", { serverId: "s1" });
    expect(state.trustedServerIds.value).toContain("s1");
  });

  it("does not duplicate trusted server id", async () => {
    const { state, actions } = setup();
    state.trustedServerIds.value = ["s1"];
    mockedInvoke.mockResolvedValueOnce(undefined);

    await actions.trustServer("s1");

    expect(state.trustedServerIds.value.filter((id) => id === "s1")).toHaveLength(1);
  });

  it("notifies on error", async () => {
    const { actions } = setup();
    mockedInvoke.mockRejectedValueOnce(new Error("trust failed"));

    await actions.trustServer("s1");

    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("trust failed")
    );
  });
});

describe("revokeTrust", () => {
  it("invokes revoke_mcp_trust and removes from trusted list", async () => {
    const { state, actions } = setup();
    state.trustedServerIds.value = ["s1", "s2"];
    mockedInvoke.mockResolvedValueOnce(undefined);

    await actions.revokeTrust("s1");

    expect(mockedInvoke).toHaveBeenCalledWith("revoke_mcp_trust", { serverId: "s1" });
    expect(state.trustedServerIds.value).not.toContain("s1");
    expect(state.trustedServerIds.value).toContain("s2");
  });

  it("notifies on error", async () => {
    const { actions } = setup();
    mockedInvoke.mockRejectedValueOnce(new Error("revoke failed"));

    await actions.revokeTrust("s1");

    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("revoke failed")
    );
  });
});

describe("refreshTools", () => {
  it("updates server health with tools on success", async () => {
    const { state, actions, updateToolCount, loadDisabledTools } = setup();
    const tools = [{ name: "read_file", description: "Read a file", input_schema: {} }];
    mockedInvoke.mockResolvedValueOnce(tools);

    await actions.refreshTools("s1");

    expect(mockedInvoke).toHaveBeenCalledWith("refresh_mcp_tools", { serverId: "s1" });
    expect(state.serverHealth.value.s1).toEqual({ tools, healthy: true, error: null });
    expect(updateToolCount).toHaveBeenCalledWith("s1", 1);
    expect(loadDisabledTools).toHaveBeenCalledWith("s1");
  });

  it("sets unhealthy state on error", async () => {
    const { state, actions } = setup();
    mockedInvoke.mockRejectedValueOnce(new Error("connection refused"));

    await actions.refreshTools("s1");

    expect(state.serverHealth.value.s1).toEqual({
      tools: [],
      healthy: false,
      error: "Error: connection refused"
    });
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("connection refused")
    );
  });
});

describe("handleMcpEvent", () => {
  it("handles McpServerStarting by adding/updating server", () => {
    const { state, actions } = setup();

    actions.handleMcpEvent({ type: "McpServerStarting", server_id: "s1" });

    expect(state.servers.value).toHaveLength(1);
    expect(state.servers.value[0].status).toBe("starting");
  });

  it("handles McpServerReady by setting running status", () => {
    const { state, actions } = setup();
    state.servers.value = [{ id: "s1", status: "starting", tool_count: null }];

    actions.handleMcpEvent({ type: "McpServerReady", server_id: "s1", tool_count: 5 });

    expect(state.servers.value[0].status).toBe("running");
    expect(state.servers.value[0].tool_count).toBe(5);
  });

  it("handles McpServerStopped by setting stopped status", () => {
    const { state, actions } = setup();
    state.servers.value = [{ id: "s1", status: "running", tool_count: 3 }];

    actions.handleMcpEvent({ type: "McpServerStopped", server_id: "s1" });

    expect(state.servers.value[0].status).toBe("stopped");
    expect(state.servers.value[0].tool_count).toBeNull();
  });

  it("handles McpServerFailed by setting failed status with error", () => {
    const { state, actions } = setup();

    actions.handleMcpEvent({
      type: "McpServerFailed",
      server_id: "s1",
      error: "connection refused"
    });

    expect(state.servers.value[0].status).toBe("failed");
    expect(state.servers.value[0].error).toBe("connection refused");
  });

  it("handles McpTrustGranted by adding to trusted list", () => {
    const { state, actions } = setup();

    actions.handleMcpEvent({ type: "McpTrustGranted", server_id: "s1" });

    expect(state.trustedServerIds.value).toContain("s1");
  });

  it("handles McpTrustRevoked by removing from trusted list", () => {
    const { state, actions } = setup();
    state.trustedServerIds.value = ["s1"];

    actions.handleMcpEvent({ type: "McpTrustRevoked", server_id: "s1" });

    expect(state.trustedServerIds.value).not.toContain("s1");
  });

  it("adds new server entry for unknown server_id", () => {
    const { state, actions } = setup();

    actions.handleMcpEvent({ type: "McpServerStarting", server_id: "new_server" });

    expect(state.servers.value).toHaveLength(1);
    expect(state.servers.value[0].id).toBe("new_server");
  });
});
