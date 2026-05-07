import { describe, it, expect, beforeEach, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

vi.mock("../composables/useNotifications", () => ({
  addNotification: vi.fn(),
  dismissNotification: vi.fn(),
  notifications: []
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import {
  mcpState,
  runningServers,
  failedServers,
  runningCount,
  hasServers,
  fetchServers,
  startServer,
  stopServer,
  trustServer,
  revokeTrust,
  refreshTools,
  handleMcpEvent
} from "./mcp";

beforeEach(() => {
  mcpState.servers = [];
  mcpState.trustedServerIds = [];
  mcpState.loading = false;
  vi.clearAllMocks();
});

describe("mcpState", () => {
  it("starts with empty servers and trusted list", () => {
    expect(mcpState.servers).toHaveLength(0);
    expect(mcpState.trustedServerIds).toHaveLength(0);
    expect(mcpState.loading).toBe(false);
  });
});

describe("computed properties", () => {
  it("runningServers filters for running status", () => {
    mcpState.servers = [
      { id: "s1", status: "running", tool_count: 3 },
      { id: "s2", status: "stopped", tool_count: null },
      { id: "s3", status: "running", tool_count: 1 }
    ];
    expect(runningServers.value).toHaveLength(2);
    expect(runningServers.value.map((s) => s.id)).toEqual(["s1", "s3"]);
  });

  it("failedServers filters for failed status", () => {
    mcpState.servers = [
      { id: "s1", status: "running", tool_count: 3 },
      { id: "s2", status: "failed", tool_count: null }
    ];
    expect(failedServers.value).toHaveLength(1);
    expect(failedServers.value[0].id).toBe("s2");
  });

  it("runningCount returns count of running servers", () => {
    mcpState.servers = [
      { id: "s1", status: "running", tool_count: 3 },
      { id: "s2", status: "stopped", tool_count: null }
    ];
    expect(runningCount.value).toBe(1);
  });

  it("hasServers returns true when servers exist", () => {
    expect(hasServers.value).toBe(false);
    mcpState.servers = [{ id: "s1", status: "stopped", tool_count: null }];
    expect(hasServers.value).toBe(true);
  });
});

describe("fetchServers", () => {
  it("populates servers from invoke result", async () => {
    mockedInvoke.mockResolvedValueOnce([
      { id: "s1", status: "running", tool_count: 3 },
      { id: "s2", status: "stopped", tool_count: null }
    ]);
    await fetchServers();
    expect(mockedInvoke).toHaveBeenCalledWith("list_mcp_servers");
    expect(mcpState.servers).toHaveLength(2);
    expect(mcpState.loading).toBe(false);
  });

  it("notifies on error", async () => {
    const { addNotification } = await import("../composables/useNotifications");
    mockedInvoke.mockRejectedValueOnce(new Error("network error"));
    await fetchServers();
    expect(addNotification).toHaveBeenCalledWith("error", expect.stringContaining("network error"));
    expect(mcpState.loading).toBe(false);
  });
});

describe("startServer", () => {
  it("invokes start_mcp_server and refreshes list", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined); // start_mcp_server
    mockedInvoke.mockResolvedValueOnce([]); // list_mcp_servers
    await startServer("s1");
    expect(mockedInvoke).toHaveBeenCalledWith("start_mcp_server", {
      serverId: "s1"
    });
    expect(mockedInvoke).toHaveBeenCalledWith("list_mcp_servers");
  });

  it("notifies on error", async () => {
    const { addNotification } = await import("../composables/useNotifications");
    mockedInvoke.mockRejectedValueOnce(new Error("start failed"));
    await startServer("s1");
    expect(addNotification).toHaveBeenCalledWith("error", expect.stringContaining("start failed"));
  });
});

describe("stopServer", () => {
  it("invokes stop_mcp_server and refreshes list", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined); // stop_mcp_server
    mockedInvoke.mockResolvedValueOnce([]); // list_mcp_servers
    await stopServer("s1");
    expect(mockedInvoke).toHaveBeenCalledWith("stop_mcp_server", {
      serverId: "s1"
    });
  });
});

describe("trustServer", () => {
  it("invokes trust_mcp_server and adds to trusted list", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    await trustServer("s1");
    expect(mockedInvoke).toHaveBeenCalledWith("trust_mcp_server", {
      serverId: "s1"
    });
    expect(mcpState.trustedServerIds).toContain("s1");
  });

  it("does not duplicate trusted server id", async () => {
    mcpState.trustedServerIds = ["s1"];
    mockedInvoke.mockResolvedValueOnce(undefined);
    await trustServer("s1");
    expect(mcpState.trustedServerIds.filter((id) => id === "s1")).toHaveLength(1);
  });
});

describe("revokeTrust", () => {
  it("invokes revoke_mcp_trust and removes from trusted list", async () => {
    mcpState.trustedServerIds = ["s1", "s2"];
    mockedInvoke.mockResolvedValueOnce(undefined);
    await revokeTrust("s1");
    expect(mockedInvoke).toHaveBeenCalledWith("revoke_mcp_trust", {
      serverId: "s1"
    });
    expect(mcpState.trustedServerIds).not.toContain("s1");
    expect(mcpState.trustedServerIds).toContain("s2");
  });
});

describe("refreshTools", () => {
  it("invokes refresh_mcp_tools and refreshes list", async () => {
    mockedInvoke.mockResolvedValueOnce([]); // refresh_mcp_tools
    mockedInvoke.mockResolvedValueOnce([]); // list_mcp_servers
    await refreshTools("s1");
    expect(mockedInvoke).toHaveBeenCalledWith("refresh_mcp_tools", {
      serverId: "s1"
    });
  });
});

describe("handleMcpEvent", () => {
  it("handles McpServerStarting by adding/updating server", () => {
    handleMcpEvent({ type: "McpServerStarting", server_id: "s1" });
    expect(mcpState.servers).toHaveLength(1);
    expect(mcpState.servers[0].status).toBe("starting");
  });

  it("handles McpServerReady by setting running status", () => {
    mcpState.servers = [{ id: "s1", status: "starting", tool_count: null }];
    handleMcpEvent({ type: "McpServerReady", server_id: "s1", tool_count: 5 });
    expect(mcpState.servers[0].status).toBe("running");
    expect(mcpState.servers[0].tool_count).toBe(5);
  });

  it("handles McpServerStopped by setting stopped status", () => {
    mcpState.servers = [{ id: "s1", status: "running", tool_count: 3 }];
    handleMcpEvent({ type: "McpServerStopped", server_id: "s1" });
    expect(mcpState.servers[0].status).toBe("stopped");
    expect(mcpState.servers[0].tool_count).toBeNull();
  });

  it("handles McpServerFailed by setting failed status", () => {
    handleMcpEvent({
      type: "McpServerFailed",
      server_id: "s1",
      error: "connection refused"
    });
    expect(mcpState.servers[0].status).toBe("failed");
    expect(mcpState.servers[0].error).toBe("connection refused");
  });

  it("handles McpTrustGranted by adding to trusted list", () => {
    handleMcpEvent({ type: "McpTrustGranted", server_id: "s1" });
    expect(mcpState.trustedServerIds).toContain("s1");
  });

  it("handles McpTrustRevoked by removing from trusted list", () => {
    mcpState.trustedServerIds = ["s1"];
    handleMcpEvent({ type: "McpTrustRevoked", server_id: "s1" });
    expect(mcpState.trustedServerIds).not.toContain("s1");
  });

  it("adds new server when event arrives for unknown server_id", () => {
    handleMcpEvent({ type: "McpServerStarting", server_id: "new_server" });
    expect(mcpState.servers).toHaveLength(1);
    expect(mcpState.servers[0].id).toBe("new_server");
    expect(mcpState.servers[0].status).toBe("starting");
  });
});
