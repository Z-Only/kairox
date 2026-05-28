import { describe, it, expect } from "vitest";
import { createMcpState } from "./state";

describe("createMcpState", () => {
  it("initializes with empty servers list", () => {
    const state = createMcpState();
    expect(state.servers.value).toEqual([]);
  });

  it("initializes with empty trusted server IDs", () => {
    const state = createMcpState();
    expect(state.trustedServerIds.value).toEqual([]);
  });

  it("initializes loading as false", () => {
    const state = createMcpState();
    expect(state.loading.value).toBe(false);
  });

  it("initializes settings state as empty", () => {
    const state = createMcpState();
    expect(state.settingsServers.value).toEqual([]);
    expect(state.settingsLoading.value).toBe(false);
    expect(state.configFileOpening.value).toBe(false);
    expect(state.settingsError.value).toBeNull();
    expect(state.effectiveServers.value).toEqual([]);
  });

  it("initializes connectivity state as empty", () => {
    const state = createMcpState();
    expect(state.connectivityResults.value).toEqual({});
    expect(state.testingConnectivity.value).toEqual(new Set());
  });

  it("initializes health and tool state as empty", () => {
    const state = createMcpState();
    expect(state.serverHealth.value).toEqual({});
    expect(state.checkingHealth.value).toEqual(new Set());
    expect(state.expandedServers.value).toEqual(new Set());
    expect(state.disabledTools.value).toEqual({});
  });

  it("initializes resource and prompt browsing state as empty", () => {
    const state = createMcpState();
    expect(state.serverResources.value).toEqual({});
    expect(state.serverPrompts.value).toEqual({});
    expect(state.loadingResources.value).toEqual(new Set());
    expect(state.loadingPrompts.value).toEqual(new Set());
    expect(state.expandedResourceUri.value).toEqual({});
    expect(state.resourcesError.value).toEqual({});
    expect(state.promptsError.value).toEqual({});
    expect(state.resourceContentCache.value).toEqual({});
  });

  describe("computed: runningServers", () => {
    it("filters for running status", () => {
      const state = createMcpState();
      state.servers.value = [
        { id: "s1", status: "running", tool_count: 3 },
        { id: "s2", status: "stopped", tool_count: null },
        { id: "s3", status: "running", tool_count: 1 }
      ];
      expect(state.runningServers.value).toHaveLength(2);
      expect(state.runningServers.value.map((s) => s.id)).toEqual(["s1", "s3"]);
    });

    it("returns empty when no servers running", () => {
      const state = createMcpState();
      state.servers.value = [{ id: "s1", status: "stopped", tool_count: null }];
      expect(state.runningServers.value).toHaveLength(0);
    });
  });

  describe("computed: failedServers", () => {
    it("filters for failed status", () => {
      const state = createMcpState();
      state.servers.value = [
        { id: "s1", status: "running", tool_count: 3 },
        { id: "s2", status: "failed", tool_count: null }
      ];
      expect(state.failedServers.value).toHaveLength(1);
      expect(state.failedServers.value[0].id).toBe("s2");
    });
  });

  describe("computed: runningCount", () => {
    it("returns count of running servers", () => {
      const state = createMcpState();
      state.servers.value = [
        { id: "s1", status: "running", tool_count: 3 },
        { id: "s2", status: "stopped", tool_count: null },
        { id: "s3", status: "running", tool_count: 2 }
      ];
      expect(state.runningCount.value).toBe(2);
    });
  });

  describe("computed: hasServers", () => {
    it("returns false when no servers", () => {
      const state = createMcpState();
      expect(state.hasServers.value).toBe(false);
    });

    it("returns true when servers exist", () => {
      const state = createMcpState();
      state.servers.value = [{ id: "s1", status: "stopped", tool_count: null }];
      expect(state.hasServers.value).toBe(true);
    });
  });
});
