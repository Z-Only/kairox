import { describe, it, expect, beforeEach, vi } from "vitest";
import { createMcpState } from "./state";
import { createConnectivity } from "./connectivity";
import { createUiStoreMock } from "@/test-utils/uiMock";
import type { McpServerSettingsView } from "@/generated/commands";

vi.mock("@/generated/commands", () => ({
  commands: {
    testMcpConnectivity: vi.fn()
  }
}));

const pushNotificationSpy = vi.fn();
vi.mock("@/stores/ui", () => ({
  useUiStore: () => createUiStoreMock({ pushNotification: pushNotificationSpy })
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
  const actions = createConnectivity(state);
  return { state, actions };
}

describe("testConnectivity", () => {
  it("stores connected result and notifies success", async () => {
    const { state, actions } = setup();
    state.effectiveServers.value = [
      {
        value: createServerView({ id: "github", name: "GitHub" }),
        source: "User",
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      }
    ];
    mockedCommands.testMcpConnectivity.mockResolvedValueOnce({
      status: "ok",
      data: { status: "connected", tool_count: 5 }
    });

    await actions.testConnectivity("github");

    expect(mockedCommands.testMcpConnectivity).toHaveBeenCalledWith("github");
    expect(state.connectivityResults.value.github).toEqual({
      status: "connected",
      tool_count: 5
    });
    expect(pushNotificationSpy).toHaveBeenCalledWith("success", expect.stringContaining("GitHub"));
    expect(state.testingConnectivity.value.has("github")).toBe(false);
  });

  it("stores failed result and notifies error on connected failure", async () => {
    const { state, actions } = setup();
    state.effectiveServers.value = [
      {
        value: createServerView({ id: "github", name: "GitHub" }),
        source: "User",
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      }
    ];
    mockedCommands.testMcpConnectivity.mockResolvedValueOnce({
      status: "ok",
      data: { status: "failed", reason: "connection refused" }
    });

    await actions.testConnectivity("github");

    expect(state.connectivityResults.value.github).toEqual({
      status: "failed",
      reason: "connection refused"
    });
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("connection refused")
    );
  });

  it("handles command error result", async () => {
    const { state, actions } = setup();
    state.effectiveServers.value = [
      {
        value: createServerView({ id: "github", name: "GitHub" }),
        source: "User",
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      }
    ];
    mockedCommands.testMcpConnectivity.mockResolvedValueOnce({
      status: "error",
      error: "server not found"
    });

    await actions.testConnectivity("github");

    expect(state.connectivityResults.value.github).toEqual({
      status: "failed",
      reason: "server not found"
    });
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("server not found")
    );
  });

  it("handles thrown exception", async () => {
    const { state, actions } = setup();
    state.effectiveServers.value = [];
    mockedCommands.testMcpConnectivity.mockRejectedValueOnce(new Error("network error"));

    await actions.testConnectivity("github");

    expect(state.connectivityResults.value.github).toEqual({
      status: "failed",
      reason: "Error: network error"
    });
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("network error")
    );
  });

  it("tracks testing state during connectivity test", async () => {
    const { state, actions } = setup();
    let resolvePromise: (v: unknown) => void;
    const pending = new Promise((resolve) => {
      resolvePromise = resolve;
    });
    mockedCommands.testMcpConnectivity.mockReturnValueOnce(pending as any);

    const testPromise = actions.testConnectivity("s1");
    expect(state.testingConnectivity.value.has("s1")).toBe(true);

    resolvePromise!({ status: "ok", data: { status: "connected", tool_count: 0 } });
    await testPromise;
    expect(state.testingConnectivity.value.has("s1")).toBe(false);
  });

  it("uses server id as name when server not found in effective list", async () => {
    const { state, actions } = setup();
    state.effectiveServers.value = [];
    mockedCommands.testMcpConnectivity.mockResolvedValueOnce({
      status: "ok",
      data: { status: "connected", tool_count: 2 }
    });

    await actions.testConnectivity("unknown_server");

    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "success",
      expect.stringContaining("unknown_server")
    );
  });
});

describe("testAllConnectivity", () => {
  it("tests connectivity for all non-builtin servers", async () => {
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
        value: createServerView({ id: "builtin_tools", transport: "builtin" }),
        source: "User",
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      },
      {
        value: createServerView({ id: "github", transport: "sse" }),
        source: "User",
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      }
    ];
    mockedCommands.testMcpConnectivity.mockResolvedValue({
      status: "ok",
      data: { status: "connected", tool_count: 1 }
    });

    await actions.testAllConnectivity();

    // Should skip builtin transport
    expect(mockedCommands.testMcpConnectivity).toHaveBeenCalledTimes(2);
    expect(mockedCommands.testMcpConnectivity).toHaveBeenCalledWith("files");
    expect(mockedCommands.testMcpConnectivity).toHaveBeenCalledWith("github");
  });
});
