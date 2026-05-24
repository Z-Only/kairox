import { describe, it, expect, vi, beforeEach, beforeAll } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { nextTick, ref } from "vue";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import { invoke } from "@tauri-apps/api/core";
import {
  commands,
  type EffectiveMcpServerView,
  type McpServerSettingsView
} from "@/generated/commands";
import { useMcpStore } from "@/stores/mcp";
import { useProjectStore } from "@/stores/project";
import McpSettingsPane from "./McpSettingsPane.vue";
import mcpSettingsPaneSource from "./McpSettingsPane.vue?raw";
import mcpServerCardSource from "./McpServerCard.vue?raw";
import mcpResourceAccordionSource from "./McpResourceAccordion.vue?raw";
import mcpPromptAccordionSource from "./McpPromptAccordion.vue?raw";

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

const mockedInvoke = vi.mocked(invoke);
const mockedCommands = vi.mocked(commands);

beforeAll(() => {
  HTMLDialogElement.prototype.showModal ??= vi.fn();
  HTMLDialogElement.prototype.close ??= vi.fn();
});

const githubServer: McpServerSettingsView = {
  id: "github",
  name: "GitHub",
  transport: "stdio",
  enabled: true,
  runtime_status: "running",
  trusted: false,
  tool_count: 5,
  last_error: null,
  writable: true,
  config_path: "/tmp/kairox.toml",
  description: "GitHub automation"
};

const readonlyServer: McpServerSettingsView = {
  id: "builtin-docs",
  name: "Built-in docs",
  transport: "sse",
  enabled: false,
  runtime_status: "failed",
  trusted: true,
  tool_count: null,
  last_error: "connection refused",
  writable: false,
  config_path: null,
  description: "Read-only fixture"
};

function ok<T>(data: T): { status: "ok"; data: T } {
  return { status: "ok", data };
}

function toEffective(server: McpServerSettingsView): EffectiveMcpServerView {
  return {
    value: server,
    source: server.config_path ? "User" : "Builtin",
    overrides: null,
    enabled: server.enabled,
    disabledBy: null,
    writable: server.writable,
    deletable: server.writable
  };
}

function mountPane(configSource?: "user" | "project", configProjectId?: string) {
  const mountOptions: MountWithPluginsOptions<typeof McpSettingsPane> = {
    reusePinia: true,
    mount:
      configSource || configProjectId
        ? {
            global: {
              provide: {
                configSource: ref(configSource ?? "user"),
                configProjectId: ref(configProjectId)
              }
            }
          }
        : undefined
  };
  return mountWithPlugins(McpSettingsPane, mountOptions).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockedCommands.listMcpServerSettings.mockResolvedValue(ok([githubServer, readonlyServer]));
  mockedCommands.getEffectiveMcpServers.mockResolvedValue(
    ok([toEffective(githubServer), toEffective(readonlyServer)])
  );
  mockedCommands.upsertMcpServerSettings.mockResolvedValue(ok(githubServer));
  mockedCommands.setMcpServerEnabled.mockResolvedValue(ok(null));
  mockedCommands.deleteMcpServerSettings.mockResolvedValue(ok(null));
  mockedCommands.disableMcpServerAtScope.mockResolvedValue(ok(null));
  mockedCommands.enableMcpServerAtScope.mockResolvedValue(ok(null));
  mockedCommands.openMcpConfigFile.mockResolvedValue(ok("/tmp/kairox.toml"));
  mockedCommands.checkMcpHealth.mockResolvedValue(
    ok({
      tools: [{ name: "search_repos", description: "Search repositories", input_schema: {} }],
      healthy: true,
      error: null
    })
  );
  mockedCommands.getMcpToolStates.mockResolvedValue(ok({ disabled_tools: [] }));
  mockedCommands.testMcpConnectivity.mockResolvedValue(ok({ status: "connected", tool_count: 5 }));
  mockedInvoke.mockResolvedValue([]);
  mockedCommands.listMcpResources.mockResolvedValue(
    ok([
      { uri: "file://logs/app.log", name: "App Log", description: null, mime_type: "text/plain" },
      {
        uri: "file://config/settings.json",
        name: "Settings",
        description: null,
        mime_type: "application/json"
      }
    ])
  );
  mockedCommands.listMcpPrompts.mockResolvedValue(
    ok([
      { name: "analyze_code", description: "Analyze code", argument_count: 2 },
      { name: "summarize_text", description: null, argument_count: 1 }
    ])
  );
  mockedCommands.readMcpResource.mockResolvedValue(ok([{ type: "text", text: "Hello World" }]));
});

describe("McpSettingsPane", () => {
  it("renders installed servers first with status, trust state, errors, and row actions", async () => {
    const wrapper = mountPane();
    await flushPromises();

    const serversSection = wrapper.find('[data-test="mcp-installed-servers"]');
    const addButton = serversSection.find('[data-test="mcp-add-server-btn"]');

    expect(mockedCommands.listMcpServerSettings).toHaveBeenCalledTimes(1);
    expect(serversSection.exists()).toBe(true);
    expect(addButton.exists()).toBe(true);
    expect(wrapper.find('[data-test="mcp-add-server-dialog"]').exists()).toBe(false);
    expect(
      Boolean(
        serversSection.element.compareDocumentPosition(addButton.element) &
        Node.DOCUMENT_POSITION_CONTAINED_BY
      )
    ).toBe(true);
    expect(wrapper.find('[data-test="mcp-server-row-github"]').text()).toContain("GitHub");
    expect(wrapper.find('[data-test="mcp-server-search-input"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="mcp-server-list"]').classes()).toContain("settings-card-list");
    expect(wrapper.find('[data-test="mcp-server-row-github"]').classes()).toContain(
      "settings-card-item"
    );
    expect(wrapper.find('[data-test="mcp-server-row-github"]').text()).toContain("1 tool");
    expect(mockedCommands.checkMcpHealth).toHaveBeenCalledWith("github");
    expect(wrapper.find('[data-test="mcp-trust-github"]').text()).toContain("Trust");
    expect(wrapper.find('[data-test="mcp-server-row-builtin-docs"]').text()).toContain(
      "connection refused"
    );
    expect(wrapper.find('[data-test="mcp-delete-github"]').exists()).toBe(true);
  });

  it("filters installed servers by search text", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="mcp-server-search-input"]').setValue("docs");

    expect(wrapper.find('[data-test="mcp-server-row-builtin-docs"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="mcp-server-row-github"]').exists()).toBe(false);
  });

  it("matches installed server search against metadata", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="mcp-server-search-input"]').setValue("failed");

    expect(wrapper.find('[data-test="mcp-server-row-builtin-docs"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="mcp-server-row-github"]').exists()).toBe(false);
  });

  it("shows a filtered empty state when no installed servers match search", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="mcp-server-search-input"]').setValue("does-not-exist");

    const empty = wrapper.find('[data-test="mcp-server-filter-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("No MCP servers match your search.");
    expect(wrapper.find('[data-test="mcp-server-list"]').exists()).toBe(false);
  });

  it("renders source, disabled-by, and effective audit state for server rows", async () => {
    mockedCommands.getEffectiveMcpServers.mockResolvedValueOnce(
      ok([
        {
          ...toEffective(githubServer),
          enabled: false,
          disabledBy: "Project",
          overrides: "User"
        }
      ])
    );

    const wrapper = mountPane("project", "project-1");
    await flushPromises();

    const audit = wrapper.find('[data-test="mcp-audit-github"]');
    expect(audit.exists()).toBe(true);
    expect(audit.text()).toContain("Source");
    expect(audit.text()).toContain("User");
    expect(audit.text()).toContain("State");
    expect(audit.text()).toContain("Disabled");
    expect(audit.text()).toContain("Effective");
    expect(audit.text()).toContain("Inactive");
    expect(audit.text()).toContain("Overrides");
    expect(audit.text()).toContain("Disabled by");
    expect(audit.text()).toContain("Project");
  });

  it("uses shared card content hierarchy for server rows", () => {
    expectSourceMigration(mcpServerCardSource, {
      required: ["SettingsItemSummary", "SettingsStatusTag"],
      forbidden: [
        ".mcp-settings__server-main",
        ".server__tags",
        "tag-success",
        "tag-warning",
        "tag-danger",
        "tag--source",
        "tag--override",
        "tag--disabled-by"
      ]
    });
  });

  it("does not keep MCP pane aria chrome inline in the component source", () => {
    expectSourceMigration(mcpSettingsPaneSource, {
      forbidden: ['aria-label="MCP settings"', 'aria-label="MCP sections"']
    });
  });

  it("labels the config action as a file opener and delegates to the MCP store", async () => {
    const wrapper = mountPane();
    await flushPromises();

    const openConfigButton = wrapper.find('[data-test="mcp-open-config"]');
    expect(openConfigButton.text()).toContain("Open Config File");

    await openConfigButton.trigger("click");
    await flushPromises();

    expect(mockedCommands.openMcpConfigFile).toHaveBeenCalledTimes(1);
  });

  it("does not show the config file action as opening while settings are loading", async () => {
    let resolveSettings: (value: { status: "ok"; data: McpServerSettingsView[] }) => void;
    mockedCommands.listMcpServerSettings.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveSettings = resolve;
      })
    );

    const wrapper = mountPane();
    await nextTick();

    const openConfigButton = wrapper.find<HTMLButtonElement>('[data-test="mcp-open-config"]');
    expect(openConfigButton.text()).toContain("Open Config File");
    expect(openConfigButton.element.disabled).toBe(false);

    resolveSettings!(ok([githubServer, readonlyServer]));
    await flushPromises();
  });

  it("disables the config file action while the file is opening", async () => {
    let resolveOpenConfig: (value: { status: "ok"; data: string }) => void;
    mockedCommands.openMcpConfigFile.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveOpenConfig = resolve;
      })
    );
    const wrapper = mountPane();
    await flushPromises();

    const openConfigButton = wrapper.find<HTMLButtonElement>('[data-test="mcp-open-config"]');
    await openConfigButton.trigger("click");
    await nextTick();

    expect(openConfigButton.text()).toContain("Opening…");
    expect(openConfigButton.element.disabled).toBe(true);

    resolveOpenConfig!(ok("/tmp"));
    await flushPromises();

    expect(openConfigButton.text()).toContain("Open Config File");
    expect(openConfigButton.element.disabled).toBe(false);
  });

  it("shows a page-level error when opening the config file fails", async () => {
    mockedCommands.openMcpConfigFile.mockRejectedValueOnce(new Error("file open denied"));
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="mcp-open-config"]').trigger("click");
    await flushPromises();

    expect(useMcpStore().settingsError).toBe("Unable to open MCP config file: file open denied");
    const error = wrapper.find('[data-test="mcp-page-error"]');
    expect(error.classes()).toContain("settings-state");
    expect(error.text()).toContain("Unable to open MCP config file: file open denied");
  });

  it("opens the add server dialog via dropdown and saves manual stdio settings", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="mcp-add-server-btn"]').trigger("click");
    expect(wrapper.find('[data-test="mcp-add-server-menu"]').exists()).toBe(true);

    await wrapper.find('[data-test="mcp-add-server-manual"]').trigger("click");
    await nextTick();
    expect(wrapper.find('[data-test="mcp-add-server-dialog"]').exists()).toBe(true);

    await wrapper.find('[data-test="mcp-form-name"]').setValue("GitHub");
    await wrapper.find('[data-test="mcp-form-command"]').setValue("npx");
    await wrapper
      .find('[data-test="mcp-form-args"]')
      .setValue("-y @modelcontextprotocol/server-github");
    await wrapper.find('[data-test="mcp-save"]').trigger("submit");
    await flushPromises();

    expect(mockedCommands.upsertMcpServerSettings).toHaveBeenCalledWith({
      name: "GitHub",
      transport: {
        transport: "stdio",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-github"],
        env: {}
      },
      enabled: true,
      description: null
    });
  });

  it("disables delete for read-only backend rows", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(
      wrapper.find<HTMLButtonElement>('[data-test="mcp-delete-builtin-docs"]').element.disabled
    ).toBe(true);
  });

  it("swaps project-scope disable and enable actions for user MCP servers", async () => {
    const projectStore = useProjectStore();
    projectStore.projects = [
      {
        projectId: "project-1",
        displayName: "Project",
        rootPath: "/tmp/project",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];

    let disabledByProject = false;
    mockedCommands.getEffectiveMcpServers.mockImplementation(async () =>
      ok([
        {
          ...toEffective(githubServer),
          enabled: !disabledByProject,
          disabledBy: disabledByProject ? "Project" : null
        },
        toEffective(readonlyServer)
      ])
    );
    mockedCommands.disableMcpServerAtScope.mockImplementation(async () => {
      disabledByProject = true;
      return ok(null);
    });
    mockedCommands.enableMcpServerAtScope.mockImplementation(async () => {
      disabledByProject = false;
      return ok(null);
    });

    const wrapper = mountPane("project", "project-1");
    await flushPromises();

    await wrapper.find('[data-test="mcp-disable-scope-github"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.disableMcpServerAtScope).toHaveBeenCalledWith("github", "/tmp/project");
    expect(wrapper.find('[data-test="mcp-server-row-github"]').text()).toContain(
      "Disabled by Project"
    );
    expect(wrapper.find('[data-test="mcp-disable-scope-github"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="mcp-enable-scope-github"]').exists()).toBe(true);

    await wrapper.find('[data-test="mcp-enable-scope-github"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.enableMcpServerAtScope).toHaveBeenCalledWith("github", "/tmp/project");
    expect(wrapper.find('[data-test="mcp-server-row-github"]').text()).not.toContain(
      "Disabled by Project"
    );
    expect(wrapper.find('[data-test="mcp-enable-scope-github"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="mcp-disable-scope-github"]').exists()).toBe(true);
  });

  it("tests server connectivity from the installed server row and shows the result", async () => {
    const wrapper = mountPane();
    await flushPromises();

    const testButton = wrapper.find('[data-test="mcp-test-connectivity-github"]');
    expect(testButton.exists()).toBe(true);
    expect(testButton.text()).toContain("Test Connectivity");

    await testButton.trigger("click");
    await flushPromises();

    expect(mockedCommands.testMcpConnectivity).toHaveBeenCalledWith("github");
    expect(wrapper.find('[data-test="mcp-connectivity-github"]').text()).toContain(
      "Connected (5 tools)"
    );
  });

  it("shows page-level errors from the MCP store", async () => {
    mockedCommands.listMcpServerSettings.mockRejectedValueOnce(new Error("settings unavailable"));
    const wrapper = mountPane();
    await flushPromises();

    expect(useMcpStore().settingsError).toBe("settings unavailable");
    const error = wrapper.find('[role="alert"]');
    expect(error.classes()).toContain("settings-state");
    expect(error.text()).toContain("settings unavailable");
  });

  it("refreshes settings, effective servers, and tools when refreshing all", async () => {
    const wrapper = mountPane();
    await flushPromises();
    vi.clearAllMocks();
    mockedCommands.listMcpServerSettings.mockResolvedValue(ok([githubServer, readonlyServer]));
    mockedCommands.getEffectiveMcpServers.mockResolvedValue(
      ok([toEffective(githubServer), toEffective({ ...readonlyServer, enabled: true })])
    );
    mockedInvoke
      .mockResolvedValueOnce([
        { name: "create_issue", description: "Create issue", input_schema: {} }
      ])
      .mockResolvedValueOnce({ disabled_tools: [] })
      .mockResolvedValueOnce([
        { name: "readonly_search", description: "Search docs", input_schema: {} }
      ])
      .mockResolvedValueOnce({ disabled_tools: [] });

    await wrapper.find('[data-test="mcp-refresh-all"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.listMcpServerSettings).toHaveBeenCalledTimes(1);
    expect(mockedCommands.getEffectiveMcpServers).toHaveBeenCalledTimes(1);
    expect(mockedInvoke).toHaveBeenCalledWith("refresh_mcp_tools", { serverId: "github" });
    expect(mockedInvoke).toHaveBeenCalledWith("refresh_mcp_tools", { serverId: "builtin-docs" });
    expect(wrapper.find('[data-test="mcp-tools-github"]').text()).toContain("1 tool");
  });

  it("shows resources accordion for non-builtin servers and loads on expand", async () => {
    const wrapper = mountPane();
    await flushPromises();

    const resourcesSection = wrapper.find('[data-test="mcp-resources-github"]');
    expect(resourcesSection.exists()).toBe(true);

    // Click toggle to expand
    const toggle = wrapper.find('[data-test="mcp-resources-toggle-github"]');
    await toggle.trigger("click");
    await flushPromises();

    expect(mockedCommands.listMcpResources).toHaveBeenCalledWith("github");
    expect(wrapper.find('[data-test="mcp-resources-list-github"]').classes()).toContain(
      "kx-accordion-list"
    );
    expect(wrapper.find('[data-test="mcp-resource-github-App Log"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="mcp-resource-github-App Log"]').classes()).toContain(
      "kx-accordion-item"
    );
    expect(wrapper.find('[data-test="mcp-resource-github-Settings"]').exists()).toBe(true);
  });

  it("shows inline content when clicking a resource row", async () => {
    const wrapper = mountPane();
    await flushPromises();

    // Expand resources accordion
    await wrapper.find('[data-test="mcp-resources-toggle-github"]').trigger("click");
    await flushPromises();

    // Click a resource row
    await wrapper.find('[data-test="mcp-resource-github-App Log"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.readMcpResource).toHaveBeenCalledWith("github", "file://logs/app.log");
    const content = wrapper.find('[data-test="mcp-resource-content-github-App Log"]');
    expect(content.exists()).toBe(true);
    expect(content.find(".content-block__text").text()).toBe("Hello World");
  });

  it("shows prompts accordion with name, args, and description", async () => {
    const wrapper = mountPane();
    await flushPromises();

    const promptsSection = wrapper.find('[data-test="mcp-prompts-github"]');
    expect(promptsSection.exists()).toBe(true);

    // Expand prompts
    await wrapper.find('[data-test="mcp-prompts-toggle-github"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.listMcpPrompts).toHaveBeenCalledWith("github");
    const promptRow = wrapper.find('[data-test="mcp-prompt-github-analyze_code"]');
    expect(wrapper.find('[data-test="mcp-prompts-list-github"]').classes()).toContain(
      "kx-accordion-list"
    );
    expect(promptRow.classes()).toContain("kx-accordion-item");
    expect(promptRow.text()).toContain("analyze_code");
    expect(promptRow.text()).toContain("2 args");
    expect(promptRow.text()).toContain("Analyze code");
  });

  it("uses shared nested accordion list, row, and state components", () => {
    for (const source of [mcpResourceAccordionSource, mcpPromptAccordionSource]) {
      expectSourceMigration(source, {
        required: ["KxAccordionList", "KxAccordionItem", "KxAccordionState"],
        forbidden: ["<KxStateBlock"]
      });
    }

    expectSourceMigration(mcpResourceAccordionSource, {
      forbidden: [".mcp-resources-list {", ".mcp-resources-row {"]
    });
    expectSourceMigration(mcpPromptAccordionSource, {
      forbidden: [".mcp-prompts-list {", ".mcp-prompts-row {"]
    });
  });

  it("uses shared settings toolbar and subtabs instead of local MCP chrome", () => {
    expectSourceMigration(mcpSettingsPaneSource, {
      required: ["SettingsFilterBar", "SettingsSubtabs", "SettingsToolbar"],
      forbidden: [
        'class="mcp-sub-tabs"',
        'class="mcp-toolbar"',
        ".mcp-sub-tabs {",
        ".mcp-toolbar {",
        ".sub-tab-btn {"
      ]
    });
  });
});
