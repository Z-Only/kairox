import { describe, it, expect, beforeEach, vi } from "vitest";
import { createMcpState } from "./state";
import { createResources } from "./resources";

vi.mock("@/generated/commands", () => ({
  commands: {
    listMcpResources: vi.fn(),
    listMcpPrompts: vi.fn(),
    readMcpResource: vi.fn()
  }
}));

import { commands } from "@/generated/commands";

const mockedCommands = vi.mocked(commands);

beforeEach(() => {
  vi.clearAllMocks();
});

function setup() {
  const state = createMcpState();
  const actions = createResources(state);
  return { state, actions };
}

describe("fetchResources", () => {
  it("fetches and caches resource list per server", async () => {
    const { state, actions } = setup();
    const mockResources = [
      { uri: "file://logs/app.log", name: "App Log", description: null, mime_type: "text/plain" }
    ];
    mockedCommands.listMcpResources.mockResolvedValueOnce({
      status: "ok",
      data: mockResources
    });

    await actions.fetchResources("github");

    expect(mockedCommands.listMcpResources).toHaveBeenCalledWith("github");
    expect(state.serverResources.value.github).toEqual(mockResources);
    expect(state.resourcesError.value.github).toBeNull();
    expect(state.loadingResources.value.has("github")).toBe(false);
  });

  it("skips fetch when resources already cached", async () => {
    const { state, actions } = setup();
    state.serverResources.value = {
      github: [{ uri: "file://x", name: "X", description: null, mime_type: null }]
    };

    await actions.fetchResources("github");

    expect(mockedCommands.listMcpResources).not.toHaveBeenCalled();
  });

  it("sets error state on error result", async () => {
    const { state, actions } = setup();
    mockedCommands.listMcpResources.mockResolvedValueOnce({
      status: "error",
      error: "not connected"
    });

    await actions.fetchResources("github");

    expect(state.resourcesError.value.github).toBe("not connected");
    expect(state.serverResources.value.github).toBeUndefined();
  });

  it("sets error state on thrown exception", async () => {
    const { state, actions } = setup();
    mockedCommands.listMcpResources.mockRejectedValueOnce(new Error("timeout"));

    await actions.fetchResources("github");

    expect(state.resourcesError.value.github).toBe("Error: timeout");
  });

  it("tracks loading state during fetch", async () => {
    const { state, actions } = setup();
    let resolvePromise: (v: unknown) => void;
    const pending = new Promise((resolve) => {
      resolvePromise = resolve;
    });
    mockedCommands.listMcpResources.mockReturnValueOnce(pending as any);

    const fetchPromise = actions.fetchResources("server1");
    expect(state.loadingResources.value.has("server1")).toBe(true);

    resolvePromise!({ status: "ok", data: [] });
    await fetchPromise;
    expect(state.loadingResources.value.has("server1")).toBe(false);
  });
});

describe("fetchPrompts", () => {
  it("fetches and caches prompt list per server", async () => {
    const { state, actions } = setup();
    const mockPrompts = [{ name: "analyze", description: "Analyze code", argument_count: 2 }];
    mockedCommands.listMcpPrompts.mockResolvedValueOnce({ status: "ok", data: mockPrompts });

    await actions.fetchPrompts("github");

    expect(mockedCommands.listMcpPrompts).toHaveBeenCalledWith("github");
    expect(state.serverPrompts.value.github).toEqual(mockPrompts);
    expect(state.promptsError.value.github).toBeNull();
  });

  it("skips fetch when prompts already cached", async () => {
    const { state, actions } = setup();
    state.serverPrompts.value = { github: [{ name: "a", description: null, argument_count: 0 }] };

    await actions.fetchPrompts("github");

    expect(mockedCommands.listMcpPrompts).not.toHaveBeenCalled();
  });

  it("sets error state on error result", async () => {
    const { state, actions } = setup();
    mockedCommands.listMcpPrompts.mockResolvedValueOnce({
      status: "error",
      error: "disconnected"
    });

    await actions.fetchPrompts("github");

    expect(state.promptsError.value.github).toBe("disconnected");
  });

  it("sets error state on thrown exception", async () => {
    const { state, actions } = setup();
    mockedCommands.listMcpPrompts.mockRejectedValueOnce(new Error("network error"));

    await actions.fetchPrompts("github");

    expect(state.promptsError.value.github).toBe("Error: network error");
  });

  it("tracks loading state during fetch", async () => {
    const { state, actions } = setup();
    let resolvePromise: (v: unknown) => void;
    const pending = new Promise((resolve) => {
      resolvePromise = resolve;
    });
    mockedCommands.listMcpPrompts.mockReturnValueOnce(pending as any);

    const fetchPromise = actions.fetchPrompts("server1");
    expect(state.loadingPrompts.value.has("server1")).toBe(true);

    resolvePromise!({ status: "ok", data: [] });
    await fetchPromise;
    expect(state.loadingPrompts.value.has("server1")).toBe(false);
  });
});

describe("readResource", () => {
  it("reads resource content and caches it", async () => {
    const { state, actions } = setup();
    const contentBlocks = [{ type: "text" as const, text: "Hello World" }];
    mockedCommands.readMcpResource.mockResolvedValueOnce({ status: "ok", data: contentBlocks });

    const result = await actions.readResource("github", "file://logs/app.log");

    expect(mockedCommands.readMcpResource).toHaveBeenCalledWith("github", "file://logs/app.log");
    expect(result).toEqual(contentBlocks);
    expect(state.resourceContentCache.value["github:file://logs/app.log"]).toEqual(contentBlocks);
  });

  it("returns cached content on second call", async () => {
    const { state, actions } = setup();
    const contentBlocks = [{ type: "text" as const, text: "cached" }];
    state.resourceContentCache.value = { "github:file://logs/app.log": contentBlocks };

    const result = await actions.readResource("github", "file://logs/app.log");

    expect(mockedCommands.readMcpResource).not.toHaveBeenCalled();
    expect(result).toEqual(contentBlocks);
  });

  it("throws on error result", async () => {
    const { actions } = setup();
    mockedCommands.readMcpResource.mockResolvedValueOnce({
      status: "error",
      error: "not found"
    });

    await expect(actions.readResource("github", "file://missing")).rejects.toThrow("not found");
  });
});

describe("toggleResourceExpand", () => {
  it("sets expanded resource URI for a server", () => {
    const { state, actions } = setup();

    actions.toggleResourceExpand("github", "file://logs/app.log");

    expect(state.expandedResourceUri.value.github).toBe("file://logs/app.log");
  });

  it("clears expanded URI when toggling same resource again", () => {
    const { state, actions } = setup();
    state.expandedResourceUri.value = { github: "file://logs/app.log" };

    actions.toggleResourceExpand("github", "file://logs/app.log");

    expect(state.expandedResourceUri.value.github).toBeNull();
  });

  it("switches expanded URI when toggling different resource", () => {
    const { state, actions } = setup();
    state.expandedResourceUri.value = { github: "file://logs/app.log" };

    actions.toggleResourceExpand("github", "file://config/settings.json");

    expect(state.expandedResourceUri.value.github).toBe("file://config/settings.json");
  });
});
