import { describe, it, expect, vi, beforeEach, beforeAll } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { ref } from "vue";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import type { EffectiveMcpServerView, McpServerSettingsView } from "@/generated/commands";
import { useMcpStore } from "@/stores/mcp";
import { useProjectStore } from "@/stores/project";
import McpServerCard from "./McpServerCard.vue";
import mcpServerCardSource from "./McpServerCard.vue?raw";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("@/generated/commands", () => ({
  commands: {
    listMcpServerSettings: vi.fn(),
    upsertMcpServerSettings: vi.fn(),
    setMcpServerEnabled: vi.fn(),
    deleteMcpServerSettings: vi.fn(),
    disableMcpServerAtScope: vi.fn(),
    enableMcpServerAtScope: vi.fn(),
    openMcpConfigFile: vi.fn(),
    getEffectiveMcpServers: vi.fn(),
    checkMcpHealth: vi.fn(),
    getMcpToolStates: vi.fn(),
    listMcpResources: vi.fn(),
    listMcpPrompts: vi.fn(),
    readMcpResource: vi.fn(),
    testMcpConnectivity: vi.fn()
  }
}));

beforeAll(() => {
  HTMLDialogElement.prototype.showModal ??= vi.fn();
  HTMLDialogElement.prototype.close ??= vi.fn();
});

function makeServer(overrides: Partial<McpServerSettingsView> = {}): McpServerSettingsView {
  return {
    id: "test-server",
    name: "Test Server",
    transport: "stdio",
    enabled: true,
    runtime_status: "running",
    trusted: false,
    tool_count: 3,
    last_error: null,
    writable: true,
    config_path: "/tmp/kairox.toml",
    description: "A test MCP server",
    diagnostic_summary: "status: running; trust: untrusted; 3 tools; unverified; error: none",
    ...overrides
  };
}

function toEffective(
  server: McpServerSettingsView,
  overrides: Partial<EffectiveMcpServerView> = {}
): EffectiveMcpServerView {
  return {
    value: server,
    source: server.config_path ? "User" : "Builtin",
    overrides: null,
    enabled: server.enabled,
    disabledBy: null,
    writable: server.writable,
    deletable: server.writable,
    ...overrides
  };
}

function mountCard(
  server: EffectiveMcpServerView,
  configSource?: "user" | "project",
  configProjectId?: string
) {
  const provide: Record<string, unknown> = {};
  if (configSource) provide.configSource = ref(configSource);
  if (configProjectId !== undefined) provide.configProjectId = ref(configProjectId);

  const mountOptions: MountWithPluginsOptions<typeof McpServerCard> = {
    reusePinia: true,
    mount: {
      props: { server },
      global: {
        provide
      }
    }
  };
  return mountWithPlugins(McpServerCard, mountOptions).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("McpServerCard", () => {
  describe("source migration guard", () => {
    it("uses shared summary/status chrome without duplicating the effective audit row", () => {
      expectSourceMigration(mcpServerCardSource, {
        required: ["SettingsCardItem", "SettingsItemSummary", "SettingsStatusTag"],
        forbidden: ["SettingsEffectiveAudit", "mcp-audit-"]
      });
    });
  });

  describe("rendering", () => {
    it("renders server name and description", () => {
      const server = toEffective(makeServer());
      const wrapper = mountCard(server);
      expect(wrapper.text()).toContain("Test Server");
      expect(wrapper.text()).toContain("A test MCP server");
    });

    it("shows fallback text when description is absent", () => {
      const server = toEffective(makeServer({ description: null }));
      const wrapper = mountCard(server);
      expect(wrapper.text()).toContain("No description");
    });

    it("renders the source tag", () => {
      const server = toEffective(makeServer());
      const wrapper = mountCard(server);
      expect(wrapper.text()).toContain("User");
    });

    it("renders transport tag", () => {
      const server = toEffective(makeServer({ transport: "sse" }));
      const wrapper = mountCard(server);
      expect(wrapper.text()).toContain("sse");
    });

    it("renders enabled tag when enabled", () => {
      const server = toEffective(makeServer({ enabled: true }));
      const wrapper = mountCard(server);
      expect(wrapper.text()).toContain("Enabled");
    });

    it("renders disabled tag when disabled", () => {
      const server = toEffective(makeServer({ enabled: false }), { enabled: false });
      const wrapper = mountCard(server);
      expect(wrapper.text()).toContain("Disabled");
    });

    it("renders trusted tag when trusted", () => {
      const server = toEffective(makeServer({ trusted: true }));
      const wrapper = mountCard(server);
      expect(wrapper.text()).toContain("Trusted");
    });

    it("renders untrusted tag when not trusted", () => {
      const server = toEffective(makeServer({ trusted: false }));
      const wrapper = mountCard(server);
      expect(wrapper.text()).toContain("Untrusted");
    });

    it("renders the diagnostic summary", () => {
      const server = toEffective(
        makeServer({
          diagnostic_summary:
            "status: failed; trust: trusted; tools: unknown; verified; error: timeout"
        })
      );
      const wrapper = mountCard(server);
      const summary = wrapper.find(`[data-test="mcp-diagnostics-${server.value.id}"]`);
      expect(summary.exists()).toBe(true);
      expect(summary.text()).toContain("tools: unknown");
      expect(summary.text()).toContain("error: timeout");
    });

    it("renders overrides tag when present", () => {
      const server = toEffective(makeServer(), { overrides: "Project" });
      const wrapper = mountCard(server);
      expect(wrapper.text()).toMatch(/overrides.*Project|Project.*overrides/i);
    });

    it("renders disabledBy tag when present", () => {
      const server = toEffective(makeServer(), { disabledBy: "Project" });
      const wrapper = mountCard(server);
      expect(wrapper.text()).toMatch(/disabled.*Project|Project.*disabled/i);
    });

    it("does not render unverified tag (source uses ConfigScope casing)", () => {
      const server = toEffective(
        makeServer({ source: "builtin", verified: false, config_path: null }),
        { source: "Builtin" }
      );
      const wrapper = mountCard(server);
      expect(wrapper.text()).not.toContain("Unverified");
    });
  });

  describe("error display", () => {
    it("renders last_error when present", () => {
      const server = toEffective(makeServer({ last_error: "connection refused" }));
      const wrapper = mountCard(server);
      const errorAlert = wrapper.find(`[data-test="mcp-row-error-${server.value.id}"]`);
      expect(errorAlert.exists()).toBe(true);
      expect(errorAlert.text()).toContain("connection refused");
    });

    it("does not render error alert when last_error is null", () => {
      const server = toEffective(makeServer({ last_error: null }));
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-row-error-${server.value.id}"]`).exists()).toBe(false);
    });
  });

  describe("health status", () => {
    it("renders healthy tag when health check passes", () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.serverHealth[server.value.id] = {
        tools: [{ name: "search", description: "Search", input_schema: {} }],
        healthy: true,
        error: null
      };
      const wrapper = mountCard(server);
      const healthTag = wrapper.find(`[data-test="mcp-health-${server.value.id}"]`);
      expect(healthTag.exists()).toBe(true);
      expect(healthTag.text()).toContain("Healthy");
    });

    it("renders unhealthy tag when health check fails", () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.serverHealth[server.value.id] = {
        tools: [],
        healthy: false,
        error: "timeout"
      };
      const wrapper = mountCard(server);
      const healthTag = wrapper.find(`[data-test="mcp-health-${server.value.id}"]`);
      expect(healthTag.exists()).toBe(true);
      expect(healthTag.text()).toContain("Unhealthy");
    });

    it("renders checking health label while health check is in progress", () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.checkingHealth.add(server.value.id);
      mcp.serverHealth[server.value.id] = { tools: [], healthy: false, error: null };
      const wrapper = mountCard(server);
      expect(wrapper.text()).toContain("Checking");
    });

    it("does not render health tag for builtin transport", () => {
      const server = toEffective(makeServer({ transport: "builtin" }));
      const mcp = useMcpStore();
      mcp.serverHealth[server.value.id] = { tools: [], healthy: true, error: null };
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-health-${server.value.id}"]`).exists()).toBe(false);
    });
  });

  describe("connectivity", () => {
    it("renders connected label with tool count", () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.connectivityResults[server.value.id] = { status: "connected", tool_count: 5 };
      const wrapper = mountCard(server);
      const tag = wrapper.find(`[data-test="mcp-connectivity-${server.value.id}"]`);
      expect(tag.exists()).toBe(true);
      expect(tag.text()).toMatch(/connected|5/i);
    });

    it("renders failed connectivity label with reason", () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.connectivityResults[server.value.id] = {
        status: "failed",
        reason: "ECONNREFUSED"
      } as any;
      const wrapper = mountCard(server);
      const tag = wrapper.find(`[data-test="mcp-connectivity-${server.value.id}"]`);
      expect(tag.exists()).toBe(true);
      expect(tag.text()).toContain("ECONNREFUSED");
    });

    it("does not render connectivity tag for builtin transport", () => {
      const server = toEffective(makeServer({ transport: "builtin" }));
      const mcp = useMcpStore();
      mcp.connectivityResults[server.value.id] = { status: "connected", tool_count: 3 };
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-connectivity-${server.value.id}"]`).exists()).toBe(
        false
      );
    });
  });

  describe("action buttons", () => {
    it("renders recheck health button for non-builtin servers", () => {
      const server = toEffective(makeServer());
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-recheck-${server.value.id}"]`).exists()).toBe(true);
    });

    it("does not render recheck health button for builtin transport", () => {
      const server = toEffective(makeServer({ transport: "builtin" }));
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-recheck-${server.value.id}"]`).exists()).toBe(false);
    });

    it("renders test connectivity button for non-builtin servers", () => {
      const server = toEffective(makeServer());
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-test-connectivity-${server.value.id}"]`).exists()).toBe(
        true
      );
    });

    it("does not render test connectivity button for builtin transport", () => {
      const server = toEffective(makeServer({ transport: "builtin" }));
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-test-connectivity-${server.value.id}"]`).exists()).toBe(
        false
      );
    });

    it("renders enable/disable toggle", () => {
      const server = toEffective(makeServer({ enabled: true }));
      const wrapper = mountCard(server);
      const btn = wrapper.find(`[data-test="mcp-enable-${server.value.id}"]`);
      expect(btn.exists()).toBe(true);
      expect(btn.text()).toContain("Disable");
    });

    it("shows Enable text when server is disabled", () => {
      const server = toEffective(makeServer({ enabled: false }), { enabled: false });
      const wrapper = mountCard(server);
      const btn = wrapper.find(`[data-test="mcp-enable-${server.value.id}"]`);
      expect(btn.text()).toContain("Enable");
    });

    it("renders trust button when untrusted", () => {
      const server = toEffective(makeServer({ trusted: false }));
      const wrapper = mountCard(server);
      const btn = wrapper.find(`[data-test="mcp-trust-${server.value.id}"]`);
      expect(btn.exists()).toBe(true);
      expect(btn.text()).toContain("Trust");
    });

    it("renders revoke trust button when trusted", () => {
      const server = toEffective(makeServer({ trusted: true }));
      const wrapper = mountCard(server);
      const btn = wrapper.find(`[data-test="mcp-trust-${server.value.id}"]`);
      expect(btn.text()).toContain("Revoke");
    });

    it("renders delete button", () => {
      const server = toEffective(makeServer());
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-delete-${server.value.id}"]`).exists()).toBe(true);
    });

    it("disables delete button when server is not writable", () => {
      const server = toEffective(makeServer({ writable: false }), { writable: false });
      const wrapper = mountCard(server);
      const btn = wrapper.find(`[data-test="mcp-delete-${server.value.id}"]`);
      expect((btn.element as HTMLButtonElement).disabled).toBe(true);
    });

    it("disables recheck button while health check is running", () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.checkingHealth.add(server.value.id);
      const wrapper = mountCard(server);
      const btn = wrapper.find(`[data-test="mcp-recheck-${server.value.id}"]`);
      expect((btn.element as HTMLButtonElement).disabled).toBe(true);
    });

    it("disables test connectivity button while connectivity test is running", () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.testingConnectivity.add(server.value.id);
      const wrapper = mountCard(server);
      const btn = wrapper.find(`[data-test="mcp-test-connectivity-${server.value.id}"]`);
      expect((btn.element as HTMLButtonElement).disabled).toBe(true);
    });
  });

  describe("action interactions", () => {
    it("calls checkHealth on recheck click", async () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      const spy = vi.spyOn(mcp, "checkHealth").mockResolvedValue(undefined as any);
      const wrapper = mountCard(server);
      await wrapper.find(`[data-test="mcp-recheck-${server.value.id}"]`).trigger("click");
      expect(spy).toHaveBeenCalledWith(server.value.id);
    });

    it("calls testConnectivity on test connectivity click", async () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      const spy = vi.spyOn(mcp, "testConnectivity").mockResolvedValue(undefined as any);
      const wrapper = mountCard(server);
      await wrapper.find(`[data-test="mcp-test-connectivity-${server.value.id}"]`).trigger("click");
      expect(spy).toHaveBeenCalledWith(server.value.id);
    });

    it("calls setServerEnabled on enable/disable click", async () => {
      const server = toEffective(makeServer({ enabled: true }));
      const mcp = useMcpStore();
      vi.spyOn(mcp, "setServerEnabled").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchSettingsServers").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchEffectiveServers").mockResolvedValue(undefined as any);
      const wrapper = mountCard(server);
      await wrapper.find(`[data-test="mcp-enable-${server.value.id}"]`).trigger("click");
      await flushPromises();
      expect(mcp.setServerEnabled).toHaveBeenCalledWith(server.value.id, false);
    });

    it("calls trustServer on trust click", async () => {
      const server = toEffective(makeServer({ trusted: false }));
      const mcp = useMcpStore();
      vi.spyOn(mcp, "trustServer").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchSettingsServers").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchEffectiveServers").mockResolvedValue(undefined as any);
      const wrapper = mountCard(server);
      await wrapper.find(`[data-test="mcp-trust-${server.value.id}"]`).trigger("click");
      await flushPromises();
      expect(mcp.trustServer).toHaveBeenCalledWith(server.value.id);
    });

    it("calls revokeTrust on revoke click", async () => {
      const server = toEffective(makeServer({ trusted: true }));
      const mcp = useMcpStore();
      vi.spyOn(mcp, "revokeTrust").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchSettingsServers").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchEffectiveServers").mockResolvedValue(undefined as any);
      const wrapper = mountCard(server);
      await wrapper.find(`[data-test="mcp-trust-${server.value.id}"]`).trigger("click");
      await flushPromises();
      expect(mcp.revokeTrust).toHaveBeenCalledWith(server.value.id);
    });

    it("calls deleteServerSettings on delete click", async () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      vi.spyOn(mcp, "deleteServerSettings").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchSettingsServers").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchEffectiveServers").mockResolvedValue(undefined as any);
      const wrapper = mountCard(server);
      await wrapper.find(`[data-test="mcp-delete-${server.value.id}"]`).trigger("click");
      await flushPromises();
      expect(mcp.deleteServerSettings).toHaveBeenCalledWith(server.value.id);
    });

    it("refreshes servers after action completes", async () => {
      const server = toEffective(makeServer({ trusted: false }));
      const mcp = useMcpStore();
      vi.spyOn(mcp, "trustServer").mockResolvedValue(undefined as any);
      const fetchSettings = vi
        .spyOn(mcp, "fetchSettingsServers")
        .mockResolvedValue(undefined as any);
      const fetchEffective = vi
        .spyOn(mcp, "fetchEffectiveServers")
        .mockResolvedValue(undefined as any);
      const wrapper = mountCard(server);
      await wrapper.find(`[data-test="mcp-trust-${server.value.id}"]`).trigger("click");
      await flushPromises();
      expect(fetchSettings).toHaveBeenCalled();
      expect(fetchEffective).toHaveBeenCalled();
    });
  });

  describe("scope-level actions", () => {
    const fakeProject = {
      projectId: "proj-1",
      displayName: "Test",
      rootPath: "/tmp/proj",
      removedAt: null,
      sortOrder: 0,
      expanded: false,
      pathExists: true
    };

    it("renders disable-in-project button when canDisableAtScope is true", () => {
      const server = toEffective(makeServer(), { source: "User" });
      const projectStore = useProjectStore();
      projectStore.projects = [fakeProject];
      const wrapper = mountCard(server, "project", "proj-1");
      expect(wrapper.find(`[data-test="mcp-disable-scope-${server.value.id}"]`).exists()).toBe(
        true
      );
    });

    it("does not render disable-in-project button in user scope", () => {
      const server = toEffective(makeServer(), { source: "User" });
      const wrapper = mountCard(server, "user");
      expect(wrapper.find(`[data-test="mcp-disable-scope-${server.value.id}"]`).exists()).toBe(
        false
      );
    });

    it("renders enable-in-project button when disabledBy is Project", () => {
      const server = toEffective(makeServer(), { disabledBy: "Project" });
      const projectStore = useProjectStore();
      projectStore.projects = [fakeProject];
      const wrapper = mountCard(server, "project", "proj-1");
      expect(wrapper.find(`[data-test="mcp-enable-scope-${server.value.id}"]`).exists()).toBe(true);
    });

    it("does not render enable-in-project button when not disabled by project", () => {
      const server = toEffective(makeServer());
      const wrapper = mountCard(server, "project", "proj-1");
      expect(wrapper.find(`[data-test="mcp-enable-scope-${server.value.id}"]`).exists()).toBe(
        false
      );
    });

    it("calls disableServerAtScope on disable-in-project click", async () => {
      const server = toEffective(makeServer(), { source: "User" });
      const mcp = useMcpStore();
      const projectStore = useProjectStore();
      projectStore.projects = [fakeProject];
      vi.spyOn(mcp, "disableServerAtScope").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchSettingsServers").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchEffectiveServers").mockResolvedValue(undefined as any);
      const wrapper = mountCard(server, "project", "proj-1");
      await wrapper.find(`[data-test="mcp-disable-scope-${server.value.id}"]`).trigger("click");
      await flushPromises();
      expect(mcp.disableServerAtScope).toHaveBeenCalledWith(server.value.id, "/tmp/proj");
    });

    it("calls enableServerAtScope on enable-in-project click", async () => {
      const server = toEffective(makeServer(), { disabledBy: "Project" });
      const mcp = useMcpStore();
      const projectStore = useProjectStore();
      projectStore.projects = [fakeProject];
      vi.spyOn(mcp, "enableServerAtScope").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchSettingsServers").mockResolvedValue(undefined as any);
      vi.spyOn(mcp, "fetchEffectiveServers").mockResolvedValue(undefined as any);
      const wrapper = mountCard(server, "project", "proj-1");
      await wrapper.find(`[data-test="mcp-enable-scope-${server.value.id}"]`).trigger("click");
      await flushPromises();
      expect(mcp.enableServerAtScope).toHaveBeenCalledWith(server.value.id, "/tmp/proj");
    });
  });

  describe("tool list", () => {
    it("renders tool count toggle for non-builtin servers with tools", () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.serverHealth[server.value.id] = {
        tools: [
          { name: "search_repos", description: "Search", input_schema: {} },
          { name: "list_issues", description: "List", input_schema: {} }
        ],
        healthy: true,
        error: null
      };
      const wrapper = mountCard(server);
      const toggle = wrapper.find(`[data-test="mcp-tools-toggle-${server.value.id}"]`);
      expect(toggle.exists()).toBe(true);
      expect(toggle.text()).toContain("2");
    });

    it("does not render tool section for builtin transport", () => {
      const server = toEffective(makeServer({ transport: "builtin" }));
      const mcp = useMcpStore();
      mcp.serverHealth[server.value.id] = {
        tools: [{ name: "t1", description: "T1", input_schema: {} }],
        healthy: true,
        error: null
      };
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-tools-${server.value.id}"]`).exists()).toBe(false);
    });

    it("does not render tool section when tool count is 0", () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.serverHealth[server.value.id] = { tools: [], healthy: true, error: null };
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-tools-${server.value.id}"]`).exists()).toBe(false);
    });

    it("expands tool list on toggle click", async () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.serverHealth[server.value.id] = {
        tools: [{ name: "search_repos", description: "Search", input_schema: {} }],
        healthy: true,
        error: null
      };
      const wrapper = mountCard(server);
      await wrapper.find(`[data-test="mcp-tools-toggle-${server.value.id}"]`).trigger("click");
      expect(mcp.expandedServers.has(server.value.id)).toBe(true);
    });

    it("renders individual tool buttons when expanded", async () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.serverHealth[server.value.id] = {
        tools: [
          { name: "search_repos", description: "Search repos", input_schema: {} },
          { name: "list_issues", description: "List issues", input_schema: {} }
        ],
        healthy: true,
        error: null
      };
      mcp.expandedServers.add(server.value.id);
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-tool-${server.value.id}-search_repos"]`).exists()).toBe(
        true
      );
      expect(wrapper.find(`[data-test="mcp-tool-${server.value.id}-list_issues"]`).exists()).toBe(
        true
      );
    });

    it("applies enabled class to non-disabled tools", () => {
      const server = toEffective(makeServer());
      const mcp = useMcpStore();
      mcp.serverHealth[server.value.id] = {
        tools: [{ name: "search_repos", description: "Search", input_schema: {} }],
        healthy: true,
        error: null
      };
      mcp.expandedServers.add(server.value.id);
      const wrapper = mountCard(server);
      const toolBtn = wrapper.find(`[data-test="mcp-tool-${server.value.id}-search_repos"]`);
      expect(toolBtn.classes()).toContain("mcp-settings__tool-btn--enabled");
    });
  });

  describe("data-test attributes", () => {
    it("sets data-test on the card root with server id", () => {
      const server = toEffective(makeServer({ id: "my-server" }));
      const wrapper = mountCard(server);
      expect(wrapper.find('[data-test="mcp-server-row-my-server"]').exists()).toBe(true);
    });

    it("does not render a duplicate audit element below summary tags", () => {
      const server = toEffective(makeServer());
      const wrapper = mountCard(server);
      expect(wrapper.find(`[data-test="mcp-audit-${server.value.id}"]`).exists()).toBe(false);
    });
  });
});
