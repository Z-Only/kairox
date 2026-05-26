import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useAgentSettingsStore } from "@/stores/agentSettings";
import type { AgentSettingsInput, AgentSettingsView } from "@/generated/commands";

vi.mock("@/generated/commands", () => ({
  commands: {
    listAgentSettings: vi.fn(),
    upsertAgentSettings: vi.fn(),
    deleteAgentSettings: vi.fn(),
    copyAgentSettings: vi.fn(),
    openAgentsDir: vi.fn()
  }
}));

import { commands } from "@/generated/commands";
const mockedCommands = vi.mocked(commands);

function ok<T>(data: T): { status: "ok"; data: T } {
  return { status: "ok", data };
}

function err(error: string): { status: "error"; error: string } {
  return { status: "error", error };
}

function makeAgent(overrides: Partial<AgentSettingsView> = {}): AgentSettingsView {
  return {
    id: "planner",
    settings_id: "user:planner",
    name: "Planner",
    description: "Plans tasks",
    role: "Planner",
    scope: "User",
    path: "/mock/agents/planner.md",
    effective: true,
    shadowed_by: null,
    ...overrides
  } as AgentSettingsView;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("useAgentSettingsStore", () => {
  it("loadAgents stores returned agents and clears loading", async () => {
    const store = useAgentSettingsStore();
    const agent = makeAgent();
    mockedCommands.listAgentSettings.mockResolvedValueOnce(ok([agent]));

    await store.loadAgents();

    expect(store.agents).toEqual([agent]);
    expect(store.loading).toBe(false);
    expect(store.error).toBeNull();
  });

  it("loadAgents records the error and clears loading when the command rejects", async () => {
    const store = useAgentSettingsStore();
    mockedCommands.listAgentSettings.mockRejectedValueOnce(new Error("offline"));

    await store.loadAgents();

    expect(store.agents).toEqual([]);
    expect(store.loading).toBe(false);
    expect(store.error).toBe("offline");
  });

  it("loadAgents formats non-Error rejections through String()", async () => {
    const store = useAgentSettingsStore();
    mockedCommands.listAgentSettings.mockRejectedValueOnce("plain failure" as never);

    await store.loadAgents();

    expect(store.error).toBe("plain failure");
  });

  it("loadAgents surfaces command-result errors as the formatted message", async () => {
    const store = useAgentSettingsStore();
    mockedCommands.listAgentSettings.mockResolvedValueOnce(err("permission denied"));

    await store.loadAgents();

    expect(store.error).toBe("permission denied");
    expect(store.agents).toEqual([]);
  });

  it("saveAgent returns the saved agent and refreshes the list", async () => {
    const store = useAgentSettingsStore();
    const saved = makeAgent({ id: "worker" });
    mockedCommands.upsertAgentSettings.mockResolvedValueOnce(ok(saved));
    mockedCommands.listAgentSettings.mockResolvedValueOnce(ok([saved]));

    const result = await store.saveAgent({} as AgentSettingsInput);

    expect(result).toEqual(saved);
    expect(store.saving).toBe(false);
    expect(store.error).toBeNull();
  });

  it("saveAgent returns null and stores the error when the command rejects", async () => {
    const store = useAgentSettingsStore();
    mockedCommands.upsertAgentSettings.mockRejectedValueOnce(new Error("conflict"));

    const result = await store.saveAgent({} as AgentSettingsInput);

    expect(result).toBeNull();
    expect(store.saving).toBe(false);
    expect(store.error).toBe("conflict");
  });

  it("openAgentsDir swallows errors silently", async () => {
    const store = useAgentSettingsStore();
    mockedCommands.openAgentsDir.mockRejectedValueOnce(new Error("opener missing"));

    await expect(store.openAgentsDir()).resolves.toBeUndefined();
    expect(store.error).toBeNull();
  });

  it("effectiveAgents filters out shadowed scopes", async () => {
    const store = useAgentSettingsStore();
    const visible = makeAgent({ id: "planner", effective: true });
    const shadowed = makeAgent({
      id: "worker",
      settings_id: "user:worker",
      effective: false,
      shadowed_by: "Project"
    });
    mockedCommands.listAgentSettings.mockResolvedValueOnce(ok([visible, shadowed]));

    await store.loadAgents();

    expect(store.effectiveAgents).toEqual([visible]);
  });
});
