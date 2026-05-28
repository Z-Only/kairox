import { describe, it, expect, beforeEach, vi } from "vitest";
import { createMcpState } from "./state";
import { createHealth } from "./health";
import type { McpServerSettingsView } from "@/generated/commands";

vi.mock("@/generated/commands", () => ({
  commands: {
    checkMcpHealth: vi.fn()
  }
}));

import { commands } from "@/generated/commands";

const mockedCommands = vi.mocked(commands);

beforeEach(() => {
  vi.clearAllMocks();
});

function createServerView(overrides: Partial<McpServerSettingsView> = {}): McpServerSettingsView {
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

function setup() {
  const state = createMcpState();
  const updateToolCount = vi.fn();
  const loadDisabledTools = vi.fn().mockResolvedValue(undefined);
  const refreshTools = vi.fn().mockResolvedValue(undefined);
  const fetchSettingsServers = vi.fn().mockResolvedValue(undefined);
  const fetchEffectiveServers = vi.fn().mockResolvedValue(undefined);
  const actions = createHealth(state, {
    updateToolCount,
    loadDisabledTools,
    refreshTools,
    fetchSettingsServers,
    fetchEffectiveServers
  });
  return {
    state,
    actions,
    updateToolCount,
    loadDisabledTools,
    refreshTools,
    fetchSettingsServers,
    fetchEffectiveServers
  };
}

describe("checkHealth", () => {
  it("populates server health on success", async () => {
    const { state, actions, updateToolCount, loadDisabledTools } = setup();
    const healthResponse = {
      tools: [{ name: "read_file", description: "Read a file", input_schema: {} }],
      healthy: true,
      error: null
    };
    mockedCommands.checkMcpHealth.mockResolvedValueOnce({ status: "ok", data: healthResponse });

    await actions.checkHealth("s1");

    expect(mockedCommands.checkMcpHealth).toHaveBeenCalledWith("s1");
    expect(state.serverHealth.value.s1).toEqual(healthResponse);
    expect(updateToolCount).toHaveBeenCalledWith("s1", 1);
    expect(loadDisabledTools).toHaveBeenCalledWith("s1");
    expect(state.checkingHealth.value.has("s1")).toBe(false);
  });

  it("sets unhealthy state on error result", async () => {
    const { state, actions } = setup();
    mockedCommands.checkMcpHealth.mockResolvedValueOnce({
      status: "error",
      error: "not running"
    });

    await actions.checkHealth("s1");

    expect(state.serverHealth.value.s1).toEqual({
      tools: [],
      healthy: false,
      error: "not running"
    });
  });

  it("sets unhealthy state on thrown exception", async () => {
    const { state, actions } = setup();
    mockedCommands.checkMcpHealth.mockRejectedValueOnce(new Error("timeout"));

    await actions.checkHealth("s1");

    expect(state.serverHealth.value.s1).toEqual({
      tools: [],
      healthy: false,
      error: "Error: timeout"
    });
  });

  it("tracks checking state during health check", async () => {
    const { state, actions } = setup();
    let resolvePromise: (v: unknown) => void;
    const pending = new Promise((resolve) => {
      resolvePromise = resolve;
    });
    mockedCommands.checkMcpHealth.mockReturnValueOnce(pending as any);

    const checkPromise = actions.checkHealth("s1");
    expect(state.checkingHealth.value.has("s1")).toBe(true);

    resolvePromise!({ status: "ok", data: { tools: [], healthy: true, error: null } });
    await checkPromise;
    expect(state.checkingHealth.value.has("s1")).toBe(false);
  });
});

describe("checkAllHealth", () => {
  it("checks health for all enabled non-builtin servers", async () => {
    const { state, actions } = setup();
    state.effectiveServers.value = [
      {
        value: createServerView({ id: "files", transport: "stdio" }),
        source: "User",
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      },
      {
        value: createServerView({ id: "builtin", transport: "builtin" }),
        source: "User",
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      },
      {
        value: createServerView({ id: "disabled", transport: "stdio" }),
        source: "User",
        overrides: null,
        enabled: false,
        disabledBy: null,
        writable: true,
        deletable: true
      }
    ];
    mockedCommands.checkMcpHealth.mockResolvedValue({
      status: "ok",
      data: { tools: [], healthy: true, error: null }
    });

    await actions.checkAllHealth();

    // Only non-builtin + enabled server should be checked
    expect(mockedCommands.checkMcpHealth).toHaveBeenCalledTimes(1);
    expect(mockedCommands.checkMcpHealth).toHaveBeenCalledWith("files");
  });
});

describe("refreshAllTools", () => {
  it("refreshes tools for all enabled non-builtin servers", async () => {
    const { state, actions, refreshTools } = setup();
    state.effectiveServers.value = [
      {
        value: createServerView({ id: "files", transport: "stdio" }),
        source: "User",
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      },
      {
        value: createServerView({ id: "builtin", transport: "builtin" }),
        source: "User",
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      }
    ];

    await actions.refreshAllTools();

    expect(refreshTools).toHaveBeenCalledTimes(1);
    expect(refreshTools).toHaveBeenCalledWith("files");
  });
});

describe("refreshInstalledServers", () => {
  it("fetches settings and effective servers then checks health", async () => {
    const { actions, fetchSettingsServers, fetchEffectiveServers } = setup();

    await actions.refreshInstalledServers(null);

    expect(fetchSettingsServers).toHaveBeenCalledWith(null);
    expect(fetchEffectiveServers).toHaveBeenCalled();
  });

  it("passes source filter to fetchSettingsServers", async () => {
    const { actions, fetchSettingsServers } = setup();

    await actions.refreshInstalledServers("User");

    expect(fetchSettingsServers).toHaveBeenCalledWith("User");
  });

  it("refreshes all tools when forceTools option is set", async () => {
    const { state, actions, refreshTools } = setup();
    state.effectiveServers.value = [
      {
        value: createServerView({ id: "files", transport: "stdio" }),
        source: "User",
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      }
    ];

    await actions.refreshInstalledServers(null, { forceTools: true });

    expect(refreshTools).toHaveBeenCalledWith("files");
  });
});
