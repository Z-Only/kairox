import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

vi.mock("@/generated/commands", () => ({
  commands: {
    listMcpServerSettings: vi.fn(),
    getEffectiveMcpServers: vi.fn(),
    upsertMcpServerSettings: vi.fn(),
    setMcpServerEnabled: vi.fn(),
    deleteMcpServerSettings: vi.fn(),
    disableMcpServerAtScope: vi.fn(),
    enableMcpServerAtScope: vi.fn(),
    openMcpConfigFile: vi.fn(),
    listMcpResources: vi.fn(),
    listMcpPrompts: vi.fn(),
    readMcpResource: vi.fn(),
    testMcpConnectivity: vi.fn(),
    checkMcpHealth: vi.fn(),
    getMcpToolStates: vi.fn(),
    setMcpToolDisabled: vi.fn()
  }
}));

vi.mock("@/stores/ui", () => ({
  useUiStore: () => ({
    notifications: [],
    pushNotification: vi.fn(),
    dismissNotification: vi.fn(),
    colorMode: "auto",
    isDark: false,
    setTheme: vi.fn(),
    locale: "en",
    setLocale: vi.fn(),
    sidebarCollapsed: false
  })
}));

import { useMcpStore } from "./index";

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("useMcpStore composition", () => {
  it("exposes state properties", () => {
    const store = useMcpStore();

    expect(store.servers).toEqual([]);
    expect(store.trustedServerIds).toEqual([]);
    expect(store.loading).toBe(false);
    expect(store.settingsServers).toEqual([]);
    expect(store.settingsLoading).toBe(false);
    expect(store.configFileOpening).toBe(false);
    expect(store.settingsError).toBeNull();
  });

  it("exposes computed properties", () => {
    const store = useMcpStore();

    expect(store.runningServers).toEqual([]);
    expect(store.failedServers).toEqual([]);
    expect(store.runningCount).toBe(0);
    expect(store.hasServers).toBe(false);
  });

  it("exposes lifecycle actions", () => {
    const store = useMcpStore();

    expect(typeof store.fetchServers).toBe("function");
    expect(typeof store.startServer).toBe("function");
    expect(typeof store.stopServer).toBe("function");
    expect(typeof store.trustServer).toBe("function");
    expect(typeof store.revokeTrust).toBe("function");
    expect(typeof store.refreshTools).toBe("function");
    expect(typeof store.handleMcpEvent).toBe("function");
  });

  it("exposes settings actions", () => {
    const store = useMcpStore();

    expect(typeof store.fetchSettingsServers).toBe("function");
    expect(typeof store.fetchEffectiveServers).toBe("function");
    expect(typeof store.saveServerSettings).toBe("function");
    expect(typeof store.setServerEnabled).toBe("function");
    expect(typeof store.deleteServerSettings).toBe("function");
    expect(typeof store.disableServerAtScope).toBe("function");
    expect(typeof store.enableServerAtScope).toBe("function");
    expect(typeof store.openConfigFile).toBe("function");
  });

  it("exposes health and tool actions", () => {
    const store = useMcpStore();

    expect(typeof store.checkHealth).toBe("function");
    expect(typeof store.checkAllHealth).toBe("function");
    expect(typeof store.isToolDisabled).toBe("function");
    expect(typeof store.setToolDisabled).toBe("function");
    expect(typeof store.toggleExpanded).toBe("function");
    expect(typeof store.refreshInstalledServers).toBe("function");
  });

  it("exposes connectivity actions", () => {
    const store = useMcpStore();

    expect(typeof store.testConnectivity).toBe("function");
    expect(typeof store.testAllConnectivity).toBe("function");
  });

  it("exposes resource and prompt actions", () => {
    const store = useMcpStore();

    expect(typeof store.fetchResources).toBe("function");
    expect(typeof store.fetchPrompts).toBe("function");
    expect(typeof store.readResource).toBe("function");
    expect(typeof store.toggleResourceExpand).toBe("function");
  });

  it("wires lifecycle updateToolCount to settings updateToolCount", async () => {
    const store = useMcpStore();
    store.settingsServers = [
      {
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
        description: null
      }
    ];

    // Simulate event that triggers updateToolCount indirectly
    store.handleMcpEvent({ type: "McpServerReady", server_id: "files", tool_count: 7 });
    expect(store.servers.find((s) => s.id === "files")?.tool_count).toBe(7);
  });
});
