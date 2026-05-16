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
import type {
  EffectiveMcpServerView,
  McpServerSettingsInput,
  McpServerSettingsView
} from "@/generated/commands";

const mockedInvoke = vi.mocked(invoke);

function createMcpServerSettings(
  overrides: Partial<McpServerSettingsView> = {}
): McpServerSettingsView {
  return {
    id: "files",
    name: "files",
    transport: "stdio",
    enabled: true,
    runtime_status: "stopped",
    trusted: false,
    tool_count: null,
    last_error: null,
    writable: true,
    config_path: "/tmp/mcp.toml",
    description: null,
    ...overrides
  };
}

function createMcpServerInput(
  overrides: Partial<McpServerSettingsInput> = {}
): McpServerSettingsInput {
  return {
    name: "files",
    transport: {
      transport: "stdio",
      command: "npx",
      args: ["-y", "@modelcontextprotocol/server-filesystem"],
      env: {}
    },
    enabled: true,
    description: null,
    ...overrides
  };
}

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
  it("invokes refresh_mcp_tools and updates health tools", async () => {
    const mcp = useMcpStore();
    mockedInvoke.mockResolvedValueOnce([
      { name: "read_file", description: "Read a file", input_schema: {} }
    ]);
    mockedInvoke.mockResolvedValueOnce({ disabled_tools: ["write_file"] });
    await mcp.refreshTools("s1");
    expect(mockedInvoke).toHaveBeenCalledWith("refresh_mcp_tools", {
      serverId: "s1"
    });
    expect(mcp.serverHealth.s1).toEqual({
      tools: [{ name: "read_file", description: "Read a file", input_schema: {} }],
      healthy: true,
      error: null
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

describe("settings servers", () => {
  it("loads MCP settings servers from the settings command", async () => {
    mockedInvoke.mockResolvedValueOnce([createMcpServerSettings()]);

    const store = useMcpStore();
    await store.fetchSettingsServers();

    expect(mockedInvoke).toHaveBeenCalledWith("list_mcp_server_settings", { sourceFilter: null });
    expect(store.settingsServers[0].id).toBe("files");
  });

  it("saves new MCP server settings from generated command envelope", async () => {
    const savedServer = createMcpServerSettings({ id: "github", name: "github" });
    const input = createMcpServerInput({ name: "github" });
    mockedInvoke.mockResolvedValueOnce(savedServer);

    const store = useMcpStore();
    const result = await store.saveServerSettings(input);

    expect(mockedInvoke).toHaveBeenCalledWith("upsert_mcp_server_settings", { input });
    expect(result).toEqual(savedServer);
    expect(store.settingsServers).toEqual([savedServer]);
    expect(store.settingsError).toBeNull();
    expect(store.settingsLoading).toBe(false);
  });

  it("overwrites existing MCP server settings on successful save", async () => {
    const originalServer = createMcpServerSettings({ id: "files", enabled: false });
    const savedServer = createMcpServerSettings({ id: "files", enabled: true });
    const input = createMcpServerInput({ enabled: true });
    mockedInvoke.mockResolvedValueOnce(savedServer);

    const store = useMcpStore();
    store.settingsServers = [originalServer, createMcpServerSettings({ id: "github" })];

    const result = await store.saveServerSettings(input);

    expect(result).toEqual(savedServer);
    expect(store.settingsServers).toEqual([savedServer, createMcpServerSettings({ id: "github" })]);
  });

  it("returns null and preserves settings servers when saving fails", async () => {
    const existingServers = [createMcpServerSettings({ id: "files" })];
    mockedInvoke.mockRejectedValueOnce("state file is read-only");

    const store = useMcpStore();
    store.settingsServers = existingServers;

    const result = await store.saveServerSettings(createMcpServerInput({ name: "blocked" }));

    expect(result).toBeNull();
    expect(store.settingsError).toContain("state file is read-only");
    expect(store.settingsServers).toEqual(existingServers);
    expect(store.settingsLoading).toBe(false);
  });

  it("updates only the target MCP server enabled flag", async () => {
    mockedInvoke.mockResolvedValueOnce(null);

    const store = useMcpStore();
    store.settingsServers = [
      createMcpServerSettings({ id: "files", enabled: false }),
      createMcpServerSettings({ id: "github", enabled: false })
    ];

    await store.setServerEnabled("files", true);

    expect(mockedInvoke).toHaveBeenCalledWith("set_mcp_server_enabled", {
      serverId: "files",
      enabled: true
    });
    expect(store.settingsServers).toEqual([
      createMcpServerSettings({ id: "files", enabled: true }),
      createMcpServerSettings({ id: "github", enabled: false })
    ]);
  });

  it("keeps enabled flags unchanged when enabling an MCP server fails", async () => {
    const existingServers = [
      createMcpServerSettings({ id: "files", enabled: false }),
      createMcpServerSettings({ id: "github", enabled: true })
    ];
    mockedInvoke.mockRejectedValueOnce(new Error("permission denied"));

    const store = useMcpStore();
    store.settingsServers = existingServers;

    await store.setServerEnabled("files", true);

    expect(store.settingsError).toContain("permission denied");
    expect(store.settingsServers).toEqual(existingServers);
  });

  it("deletes MCP server settings after successful command", async () => {
    mockedInvoke.mockResolvedValueOnce(null);

    const store = useMcpStore();
    store.settingsServers = [
      createMcpServerSettings({ id: "files" }),
      createMcpServerSettings({ id: "github" })
    ];

    await store.deleteServerSettings("files");

    expect(mockedInvoke).toHaveBeenCalledWith("delete_mcp_server_settings", {
      serverId: "files"
    });
    expect(store.settingsServers).toEqual([createMcpServerSettings({ id: "github" })]);
  });

  it("keeps MCP server settings when delete command fails", async () => {
    const existingServers = [
      createMcpServerSettings({ id: "files" }),
      createMcpServerSettings({ id: "github" })
    ];
    mockedInvoke.mockRejectedValueOnce("delete failed");

    const store = useMcpStore();
    store.settingsServers = existingServers;

    await store.deleteServerSettings("files");

    expect(store.settingsError).toContain("delete failed");
    expect(store.settingsServers).toEqual(existingServers);
  });
});

function createEffectiveMcpServer(
  overrides: Partial<EffectiveMcpServerView> = {}
): EffectiveMcpServerView {
  return {
    value: createMcpServerSettings(),
    source: "User",
    overrides: null,
    enabled: true,
    disabledBy: null,
    writable: true,
    deletable: true,
    ...overrides
  };
}

describe("effective servers", () => {
  it("fetchEffectiveServers populates effectiveServers", async () => {
    const effective = createEffectiveMcpServer();
    mockedInvoke.mockResolvedValueOnce([effective]);

    const store = useMcpStore();
    await store.fetchEffectiveServers();

    expect(mockedInvoke).toHaveBeenCalledWith("get_effective_mcp_servers");
    expect(store.effectiveServers).toHaveLength(1);
    expect(store.effectiveServers[0].source).toBe("User");
    expect(store.effectiveServers[0].enabled).toBe(true);
    expect(store.effectiveServers[0].writable).toBe(true);
  });

  it("fetchEffectiveServers stores error on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("config not available"));

    const store = useMcpStore();
    await store.fetchEffectiveServers();

    expect(store.effectiveServers).toHaveLength(0);
    expect(store.settingsError).toContain("config not available");
  });

  it("refreshInstalledServers loads configured servers and health tools on initial page load", async () => {
    const effective = createEffectiveMcpServer({ value: createMcpServerSettings({ id: "files" }) });
    mockedInvoke
      .mockResolvedValueOnce([createMcpServerSettings({ id: "files" })])
      .mockResolvedValueOnce([effective])
      .mockResolvedValueOnce({
        tools: [{ name: "list", description: "List files", input_schema: {} }],
        healthy: true,
        error: null
      })
      .mockResolvedValueOnce({ disabled_tools: [] });

    const store = useMcpStore();
    await store.refreshInstalledServers(null);

    expect(mockedInvoke).toHaveBeenCalledWith("list_mcp_server_settings", { sourceFilter: null });
    expect(mockedInvoke).toHaveBeenCalledWith("get_effective_mcp_servers");
    expect(mockedInvoke).toHaveBeenCalledWith("check_mcp_health", { serverId: "files" });
    expect(store.serverHealth.files?.tools.map((tool) => tool.name)).toEqual(["list"]);
  });

  it("refreshInstalledServers force refreshes tool lists when requested", async () => {
    const effective = createEffectiveMcpServer({ value: createMcpServerSettings({ id: "files" }) });
    mockedInvoke
      .mockResolvedValueOnce([createMcpServerSettings({ id: "files" })])
      .mockResolvedValueOnce([effective])
      .mockResolvedValueOnce([{ name: "read", description: "Read files", input_schema: {} }])
      .mockResolvedValueOnce({ disabled_tools: [] });

    const store = useMcpStore();
    await store.refreshInstalledServers(null, { forceTools: true });

    expect(mockedInvoke).toHaveBeenCalledWith("refresh_mcp_tools", { serverId: "files" });
    expect(store.serverHealth.files?.tools.map((tool) => tool.name)).toEqual(["read"]);
  });
});
