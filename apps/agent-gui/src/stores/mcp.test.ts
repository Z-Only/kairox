import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { createUiStoreMock } from "@/test-utils/uiMock";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

const pushNotificationSpy = vi.fn();
vi.mock("@/stores/ui", () => ({
  useUiStore: () => createUiStoreMock({ pushNotification: pushNotificationSpy })
}));

import { invoke } from "@tauri-apps/api/core";
import { useMcpStore } from "@/stores/mcp";

const mockedInvoke = vi.mocked(invoke);

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  pushNotificationSpy.mockClear();
});

describe("mcpState", () => {
  it("starts with empty servers and trusted list", () => {
    const mcp = useMcpStore();
    expect(mcp.servers).toHaveLength(0);
    expect(mcp.trustedServerIds).toHaveLength(0);
    expect(mcp.loading).toBe(false);
  });
});

describe("computed properties", () => {
  it("runningServers filters for running status", () => {
    const mcp = useMcpStore();
    mcp.servers = [
      { id: "s1", status: "running", tool_count: 3 },
      { id: "s2", status: "stopped", tool_count: null },
      { id: "s3", status: "running", tool_count: 1 }
    ];
    expect(mcp.runningServers).toHaveLength(2);
    expect(mcp.runningServers.map((s) => s.id)).toEqual(["s1", "s3"]);
  });

  it("failedServers filters for failed status", () => {
    const mcp = useMcpStore();
    mcp.servers = [
      { id: "s1", status: "running", tool_count: 3 },
      { id: "s2", status: "failed", tool_count: null }
    ];
    expect(mcp.failedServers).toHaveLength(1);
    expect(mcp.failedServers[0].id).toBe("s2");
  });

  it("runningCount returns count of running servers", () => {
    const mcp = useMcpStore();
    mcp.servers = [
      { id: "s1", status: "running", tool_count: 3 },
      { id: "s2", status: "stopped", tool_count: null }
    ];
    expect(mcp.runningCount).toBe(1);
  });

  it("hasServers returns true when servers exist", () => {
    const mcp = useMcpStore();
    expect(mcp.hasServers).toBe(false);
    mcp.servers = [{ id: "s1", status: "stopped", tool_count: null }];
    expect(mcp.hasServers).toBe(true);
  });
});

describe("fetchServers", () => {
  it("populates servers from invoke result", async () => {
    const mcp = useMcpStore();
    mockedInvoke.mockResolvedValueOnce([
      { id: "s1", status: "running", tool_count: 3 },
      { id: "s2", status: "stopped", tool_count: null }
    ]);
    await mcp.fetchServers();
    expect(mockedInvoke).toHaveBeenCalledWith("list_mcp_servers");
    expect(mcp.servers).toHaveLength(2);
    expect(mcp.loading).toBe(false);
  });

  it("notifies on error", async () => {
    const mcp = useMcpStore();
    mockedInvoke.mockRejectedValueOnce(new Error("network error"));
    await mcp.fetchServers();
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("network error")
    );
    expect(mcp.loading).toBe(false);
  });
});

describe("startServer", () => {
  it("invokes start_mcp_server and refreshes list", async () => {
    const mcp = useMcpStore();
    mockedInvoke.mockResolvedValueOnce(undefined);
    mockedInvoke.mockResolvedValueOnce([]);
    await mcp.startServer("s1");
    expect(mockedInvoke).toHaveBeenCalledWith("start_mcp_server", {
      serverId: "s1"
    });
    expect(mockedInvoke).toHaveBeenCalledWith("list_mcp_servers");
  });

  it("notifies on error", async () => {
    const mcp = useMcpStore();
    mockedInvoke.mockRejectedValueOnce(new Error("start failed"));
    await mcp.startServer("s1");
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("start failed")
    );
  });
});

describe("stopServer", () => {
  it("invokes stop_mcp_server and refreshes list", async () => {
    const mcp = useMcpStore();
    mockedInvoke.mockResolvedValueOnce(undefined);
    mockedInvoke.mockResolvedValueOnce([]);
    await mcp.stopServer("s1");
    expect(mockedInvoke).toHaveBeenCalledWith("stop_mcp_server", {
      serverId: "s1"
    });
  });
});

describe("trustServer", () => {
  it("invokes trust_mcp_server and adds to trusted list", async () => {
    const mcp = useMcpStore();
    mockedInvoke.mockResolvedValueOnce(undefined);
    await mcp.trustServer("s1");
    expect(mockedInvoke).toHaveBeenCalledWith("trust_mcp_server", {
      serverId: "s1"
    });
    expect(mcp.trustedServerIds).toContain("s1");
  });

  it("does not duplicate trusted server id", async () => {
    const mcp = useMcpStore();
    mcp.trustedServerIds = ["s1"];
    mockedInvoke.mockResolvedValueOnce(undefined);
    await mcp.trustServer("s1");
    expect(mcp.trustedServerIds.filter((id) => id === "s1")).toHaveLength(1);
  });
});

describe("revokeTrust", () => {
  it("invokes revoke_mcp_trust and removes from trusted list", async () => {
    const mcp = useMcpStore();
    mcp.trustedServerIds = ["s1", "s2"];
    mockedInvoke.mockResolvedValueOnce(undefined);
    await mcp.revokeTrust("s1");
    expect(mockedInvoke).toHaveBeenCalledWith("revoke_mcp_trust", {
      serverId: "s1"
    });
    expect(mcp.trustedServerIds).not.toContain("s1");
    expect(mcp.trustedServerIds).toContain("s2");
  });
});

describe("refreshTools", () => {
  it("invokes refresh_mcp_tools and refreshes list", async () => {
    const mcp = useMcpStore();
    mockedInvoke.mockResolvedValueOnce([]);
    mockedInvoke.mockResolvedValueOnce([]);
    await mcp.refreshTools("s1");
    expect(mockedInvoke).toHaveBeenCalledWith("refresh_mcp_tools", {
      serverId: "s1"
    });
  });
});

describe("handleMcpEvent", () => {
  it("handles McpServerStarting by adding/updating server", () => {
    const mcp = useMcpStore();
    mcp.handleMcpEvent({ type: "McpServerStarting", server_id: "s1" });
    expect(mcp.servers).toHaveLength(1);
    expect(mcp.servers[0].status).toBe("starting");
  });

  it("handles McpServerReady by setting running status", () => {
    const mcp = useMcpStore();
    mcp.servers = [{ id: "s1", status: "starting", tool_count: null }];
    mcp.handleMcpEvent({
      type: "McpServerReady",
      server_id: "s1",
      tool_count: 5
    });
    expect(mcp.servers[0].status).toBe("running");
    expect(mcp.servers[0].tool_count).toBe(5);
  });

  it("handles McpServerStopped by setting stopped status", () => {
    const mcp = useMcpStore();
    mcp.servers = [{ id: "s1", status: "running", tool_count: 3 }];
    mcp.handleMcpEvent({ type: "McpServerStopped", server_id: "s1" });
    expect(mcp.servers[0].status).toBe("stopped");
    expect(mcp.servers[0].tool_count).toBeNull();
  });

  it("handles McpServerFailed by setting failed status", () => {
    const mcp = useMcpStore();
    mcp.handleMcpEvent({
      type: "McpServerFailed",
      server_id: "s1",
      error: "connection refused"
    });
    expect(mcp.servers[0].status).toBe("failed");
    expect(mcp.servers[0].error).toBe("connection refused");
  });

  it("handles McpTrustGranted by adding to trusted list", () => {
    const mcp = useMcpStore();
    mcp.handleMcpEvent({ type: "McpTrustGranted", server_id: "s1" });
    expect(mcp.trustedServerIds).toContain("s1");
  });

  it("handles McpTrustRevoked by removing from trusted list", () => {
    const mcp = useMcpStore();
    mcp.trustedServerIds = ["s1"];
    mcp.handleMcpEvent({ type: "McpTrustRevoked", server_id: "s1" });
    expect(mcp.trustedServerIds).not.toContain("s1");
  });

  it("adds new server when event arrives for unknown server_id", () => {
    const mcp = useMcpStore();
    mcp.handleMcpEvent({
      type: "McpServerStarting",
      server_id: "new_server"
    });
    expect(mcp.servers).toHaveLength(1);
    expect(mcp.servers[0].id).toBe("new_server");
    expect(mcp.servers[0].status).toBe("starting");
  });
});
