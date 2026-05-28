import { describe, it, expect, beforeEach, vi } from "vitest";
import { createMcpState } from "./state";
import { createTools } from "./tools";
import { createUiStoreMock } from "@/test-utils/uiMock";

vi.mock("@/generated/commands", () => ({
  commands: {
    getMcpToolStates: vi.fn(),
    setMcpToolDisabled: vi.fn()
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

function setup() {
  const state = createMcpState();
  const actions = createTools(state);
  return { state, actions };
}

describe("loadDisabledTools", () => {
  it("populates disabled tools from command result", async () => {
    const { state, actions } = setup();
    mockedCommands.getMcpToolStates.mockResolvedValueOnce({
      status: "ok",
      data: { disabled_tools: ["write_file", "delete_file"] }
    });

    await actions.loadDisabledTools("s1");

    expect(mockedCommands.getMcpToolStates).toHaveBeenCalledWith("s1");
    expect(state.disabledTools.value.s1).toEqual(new Set(["write_file", "delete_file"]));
  });

  it("ignores errors silently", async () => {
    const { state, actions } = setup();
    mockedCommands.getMcpToolStates.mockRejectedValueOnce(new Error("fail"));

    await actions.loadDisabledTools("s1");

    expect(state.disabledTools.value.s1).toBeUndefined();
  });
});

describe("isToolDisabled", () => {
  it("returns true for disabled tool", () => {
    const { state, actions } = setup();
    state.disabledTools.value = { s1: new Set(["write_file"]) };

    expect(actions.isToolDisabled("s1", "write_file")).toBe(true);
  });

  it("returns false for enabled tool", () => {
    const { state, actions } = setup();
    state.disabledTools.value = { s1: new Set(["write_file"]) };

    expect(actions.isToolDisabled("s1", "read_file")).toBe(false);
  });

  it("returns false when server has no disabled tools", () => {
    const { actions } = setup();

    expect(actions.isToolDisabled("unknown", "read_file")).toBe(false);
  });
});

describe("setToolDisabled", () => {
  it("disables a tool and updates local state", async () => {
    const { state, actions } = setup();
    state.disabledTools.value = { s1: new Set() };
    mockedCommands.setMcpToolDisabled.mockResolvedValueOnce(undefined as any);

    await actions.setToolDisabled("s1", "write_file", true);

    expect(mockedCommands.setMcpToolDisabled).toHaveBeenCalledWith("s1", "write_file", true);
    expect(state.disabledTools.value.s1.has("write_file")).toBe(true);
  });

  it("enables a tool and updates local state", async () => {
    const { state, actions } = setup();
    state.disabledTools.value = { s1: new Set(["write_file"]) };
    mockedCommands.setMcpToolDisabled.mockResolvedValueOnce(undefined as any);

    await actions.setToolDisabled("s1", "write_file", false);

    expect(mockedCommands.setMcpToolDisabled).toHaveBeenCalledWith("s1", "write_file", false);
    expect(state.disabledTools.value.s1.has("write_file")).toBe(false);
  });

  it("notifies on error", async () => {
    const { actions } = setup();
    mockedCommands.setMcpToolDisabled.mockRejectedValueOnce(new Error("not allowed"));

    await actions.setToolDisabled("s1", "write_file", true);

    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("not allowed")
    );
  });

  it("creates new Set for server without existing disabled tools", async () => {
    const { state, actions } = setup();
    mockedCommands.setMcpToolDisabled.mockResolvedValueOnce(undefined as any);

    await actions.setToolDisabled("new_server", "tool_a", true);

    expect(state.disabledTools.value.new_server).toEqual(new Set(["tool_a"]));
  });
});

describe("toggleExpanded", () => {
  it("adds server to expanded set", () => {
    const { state, actions } = setup();

    actions.toggleExpanded("s1");

    expect(state.expandedServers.value.has("s1")).toBe(true);
  });

  it("removes server from expanded set when toggled again", () => {
    const { state, actions } = setup();
    state.expandedServers.value = new Set(["s1"]);

    actions.toggleExpanded("s1");

    expect(state.expandedServers.value.has("s1")).toBe(false);
  });

  it("preserves other expanded servers", () => {
    const { state, actions } = setup();
    state.expandedServers.value = new Set(["s1", "s2"]);

    actions.toggleExpanded("s1");

    expect(state.expandedServers.value.has("s2")).toBe(true);
  });
});
