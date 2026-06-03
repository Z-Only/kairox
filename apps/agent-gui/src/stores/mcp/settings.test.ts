import { describe, it, expect, beforeEach, vi } from "vitest";
import { createMcpState } from "./state";
import { createSettings } from "./settings";
import type { McpServerSettingsInput, McpServerSettingsView } from "@/generated/commands";

vi.mock("@/generated/commands", () => ({
  commands: {
    listMcpServerSettings: vi.fn(),
    getEffectiveMcpServers: vi.fn(),
    upsertMcpServerSettings: vi.fn(),
    setMcpServerEnabled: vi.fn(),
    deleteMcpServerSettings: vi.fn(),
    disableMcpServerAtScope: vi.fn(),
    enableMcpServerAtScope: vi.fn(),
    openMcpConfigFile: vi.fn()
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

function createInput(overrides: Partial<McpServerSettingsInput> = {}): McpServerSettingsInput {
  return {
    name: "files",
    transport: { transport: "stdio", command: "npx", args: [], env: {} },
    enabled: true,
    description: null,
    ...overrides
  };
}

function setup() {
  const state = createMcpState();
  const actions = createSettings(state);
  return { state, actions };
}

describe("upsertSettingsServer", () => {
  it("appends new server when not already present", () => {
    const { state, actions } = setup();
    const server = createServerView({ id: "github" });

    actions.upsertSettingsServer(server);

    expect(state.settingsServers.value).toEqual([server]);
  });

  it("replaces existing server with same id", () => {
    const { state, actions } = setup();
    state.settingsServers.value = [createServerView({ id: "files", enabled: false })];
    const updated = createServerView({ id: "files", enabled: true });

    actions.upsertSettingsServer(updated);

    expect(state.settingsServers.value).toEqual([updated]);
  });
});

describe("updateToolCount", () => {
  it("updates tool count on matching settings server", () => {
    const { state, actions } = setup();
    state.settingsServers.value = [createServerView({ id: "files", tool_count: null })];

    actions.updateToolCount("files", 5);

    expect(state.settingsServers.value[0].tool_count).toBe(5);
  });

  it("refreshes diagnostics when tool count changes", () => {
    const { state, actions } = setup();
    state.settingsServers.value = [
      createServerView({
        id: "files",
        runtime_status: "running",
        trusted: true,
        tool_count: null,
        verified: true,
        diagnostic_summary: "status: running; trust: trusted; tools: unknown; verified; error: none"
      })
    ];

    actions.updateToolCount("files", 2);

    expect(state.settingsServers.value[0].diagnostic_summary).toBe(
      "status: running; trust: trusted; tools: 2 tools; verified; error: none"
    );
  });

  it("updates tool count on matching effective server", () => {
    const { state, actions } = setup();
    state.effectiveServers.value = [
      {
        value: createServerView({ id: "files", tool_count: null }),
        source: "User",
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      }
    ];

    actions.updateToolCount("files", 3);

    expect(state.effectiveServers.value[0].value.tool_count).toBe(3);
  });

  it("does not modify unrelated servers", () => {
    const { state, actions } = setup();
    state.settingsServers.value = [
      createServerView({ id: "files", tool_count: 2 }),
      createServerView({ id: "github", tool_count: 4 })
    ];

    actions.updateToolCount("files", 10);

    expect(state.settingsServers.value[1].tool_count).toBe(4);
  });
});

describe("fetchSettingsServers", () => {
  it("populates settings servers from command result", async () => {
    const { state, actions } = setup();
    const servers = [createServerView()];
    mockedCommands.listMcpServerSettings.mockResolvedValueOnce({ status: "ok", data: servers });

    await actions.fetchSettingsServers();

    expect(mockedCommands.listMcpServerSettings).toHaveBeenCalledWith(null);
    expect(state.settingsServers.value).toEqual(servers);
    expect(state.settingsLoading.value).toBe(false);
  });

  it("passes source filter when provided", async () => {
    const { actions } = setup();
    mockedCommands.listMcpServerSettings.mockResolvedValueOnce({ status: "ok", data: [] });

    await actions.fetchSettingsServers("User");

    expect(mockedCommands.listMcpServerSettings).toHaveBeenCalledWith("User");
  });

  it("sets error on failure", async () => {
    const { state, actions } = setup();
    mockedCommands.listMcpServerSettings.mockResolvedValueOnce({
      status: "error",
      error: "config not found"
    });

    await actions.fetchSettingsServers();

    expect(state.settingsError.value).toContain("config not found");
    expect(state.settingsLoading.value).toBe(false);
  });
});

describe("fetchEffectiveServers", () => {
  it("populates effective servers from command result", async () => {
    const { state, actions } = setup();
    const effective = [
      {
        value: createServerView(),
        source: "User" as const,
        overrides: null,
        enabled: true,
        disabledBy: null,
        writable: true,
        deletable: true
      }
    ];
    mockedCommands.getEffectiveMcpServers.mockResolvedValueOnce({ status: "ok", data: effective });

    await actions.fetchEffectiveServers();

    expect(state.effectiveServers.value).toEqual(effective);
  });

  it("sets error on failure", async () => {
    const { state, actions } = setup();
    mockedCommands.getEffectiveMcpServers.mockResolvedValueOnce({
      status: "error",
      error: "unavailable"
    });

    await actions.fetchEffectiveServers();

    expect(state.settingsError.value).toContain("unavailable");
  });
});

describe("saveServerSettings", () => {
  it("saves and adds new server to settings list", async () => {
    const { state, actions } = setup();
    const savedServer = createServerView({ id: "github", name: "github" });
    mockedCommands.upsertMcpServerSettings.mockResolvedValueOnce({
      status: "ok",
      data: savedServer
    });
    mockedCommands.getEffectiveMcpServers.mockResolvedValueOnce({ status: "ok", data: [] });

    const result = await actions.saveServerSettings(createInput({ name: "github" }));

    expect(result).toEqual(savedServer);
    expect(state.settingsServers.value).toContainEqual(savedServer);
    expect(state.settingsError.value).toBeNull();
    expect(state.settingsLoading.value).toBe(false);
  });

  it("returns null on failure and sets error", async () => {
    const { state, actions } = setup();
    mockedCommands.upsertMcpServerSettings.mockResolvedValueOnce({
      status: "error",
      error: "read-only"
    });

    const result = await actions.saveServerSettings(createInput());

    expect(result).toBeNull();
    expect(state.settingsError.value).toContain("read-only");
    expect(state.settingsLoading.value).toBe(false);
  });
});

describe("setServerEnabled", () => {
  it("updates enabled flag on matching server", async () => {
    const { state, actions } = setup();
    state.settingsServers.value = [createServerView({ id: "files", enabled: false })];
    mockedCommands.setMcpServerEnabled.mockResolvedValueOnce({ status: "ok", data: null });

    await actions.setServerEnabled("files", true);

    expect(state.settingsServers.value[0].enabled).toBe(true);
  });

  it("does not modify servers on failure", async () => {
    const { state, actions } = setup();
    state.settingsServers.value = [createServerView({ id: "files", enabled: false })];
    mockedCommands.setMcpServerEnabled.mockResolvedValueOnce({
      status: "error",
      error: "permission denied"
    });

    await actions.setServerEnabled("files", true);

    expect(state.settingsServers.value[0].enabled).toBe(false);
    expect(state.settingsError.value).toContain("permission denied");
  });
});

describe("deleteServerSettings", () => {
  it("removes server from settings list on success", async () => {
    const { state, actions } = setup();
    state.settingsServers.value = [
      createServerView({ id: "files" }),
      createServerView({ id: "github" })
    ];
    mockedCommands.deleteMcpServerSettings.mockResolvedValueOnce({ status: "ok", data: null });

    await actions.deleteServerSettings("files");

    expect(state.settingsServers.value).toHaveLength(1);
    expect(state.settingsServers.value[0].id).toBe("github");
  });

  it("preserves settings list on failure", async () => {
    const { state, actions } = setup();
    state.settingsServers.value = [createServerView({ id: "files" })];
    mockedCommands.deleteMcpServerSettings.mockResolvedValueOnce({
      status: "error",
      error: "delete failed"
    });

    await actions.deleteServerSettings("files");

    expect(state.settingsServers.value).toHaveLength(1);
    expect(state.settingsError.value).toContain("delete failed");
  });
});

describe("disableServerAtScope", () => {
  it("calls command and refreshes effective servers", async () => {
    const { actions } = setup();
    mockedCommands.disableMcpServerAtScope.mockResolvedValueOnce({ status: "ok", data: null });
    mockedCommands.getEffectiveMcpServers.mockResolvedValueOnce({ status: "ok", data: [] });

    await actions.disableServerAtScope("files", "/project/root");

    expect(mockedCommands.disableMcpServerAtScope).toHaveBeenCalledWith("files", "/project/root");
    expect(mockedCommands.getEffectiveMcpServers).toHaveBeenCalled();
  });

  it("sets error on failure", async () => {
    const { state, actions } = setup();
    mockedCommands.disableMcpServerAtScope.mockResolvedValueOnce({
      status: "error",
      error: "no config"
    });

    await actions.disableServerAtScope("files", "/project/root");

    expect(state.settingsError.value).toContain("no config");
  });
});

describe("enableServerAtScope", () => {
  it("calls command and refreshes effective servers", async () => {
    const { actions } = setup();
    mockedCommands.enableMcpServerAtScope.mockResolvedValueOnce({ status: "ok", data: null });
    mockedCommands.getEffectiveMcpServers.mockResolvedValueOnce({ status: "ok", data: [] });

    await actions.enableServerAtScope("files", "/project/root");

    expect(mockedCommands.enableMcpServerAtScope).toHaveBeenCalledWith("files", "/project/root");
    expect(mockedCommands.getEffectiveMcpServers).toHaveBeenCalled();
  });
});

describe("openConfigFile", () => {
  it("returns file path on success", async () => {
    const { state, actions } = setup();
    mockedCommands.openMcpConfigFile.mockResolvedValueOnce({
      status: "ok",
      data: "/home/user/.config/kairox/mcp.toml"
    });

    const result = await actions.openConfigFile();

    expect(result).toBe("/home/user/.config/kairox/mcp.toml");
    expect(state.configFileOpening.value).toBe(false);
    expect(state.settingsError.value).toBeNull();
  });

  it("returns null and sets error on failure", async () => {
    const { state, actions } = setup();
    mockedCommands.openMcpConfigFile.mockResolvedValueOnce({
      status: "error",
      error: "file not found"
    });

    const result = await actions.openConfigFile();

    expect(result).toBeNull();
    expect(state.settingsError.value).toContain("file not found");
    expect(state.configFileOpening.value).toBe(false);
  });
});
