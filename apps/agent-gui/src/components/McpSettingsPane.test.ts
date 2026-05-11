import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { nextTick } from "vue";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";
import { invoke } from "@tauri-apps/api/core";
import { commands, type McpServerSettingsView } from "@/generated/commands";
import { useMcpStore } from "@/stores/mcp";
import McpSettingsPane from "./McpSettingsPane.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("@/generated/commands", () => ({
  commands: {
    listMcpServerSettings: vi.fn(),
    upsertMcpServerSettings: vi.fn(),
    setMcpServerEnabled: vi.fn(),
    deleteMcpServerSettings: vi.fn(),
    openMcpConfigFile: vi.fn()
  }
}));

const mockedInvoke = vi.mocked(invoke);
const mockedCommands = vi.mocked(commands);

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

function mountPane() {
  const mountOptions: MountWithPluginsOptions<typeof McpSettingsPane> = {
    reusePinia: true,
    mount: {
      global: {
        stubs: {
          MarketplacePane: {
            template: '<section data-test="mcp-marketplace-embedded">Marketplace catalog</section>'
          }
        }
      }
    }
  };
  return mountWithPlugins(McpSettingsPane, mountOptions).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockedCommands.listMcpServerSettings.mockResolvedValue(ok([githubServer, readonlyServer]));
  mockedCommands.upsertMcpServerSettings.mockResolvedValue(ok(githubServer));
  mockedCommands.setMcpServerEnabled.mockResolvedValue(ok(null));
  mockedCommands.deleteMcpServerSettings.mockResolvedValue(ok(null));
  mockedCommands.openMcpConfigFile.mockResolvedValue(ok("/tmp/kairox.toml"));
  mockedInvoke.mockResolvedValue([]);
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
    expect(wrapper.find('[data-test="mcp-add-server-panel"]').exists()).toBe(false);
    expect(
      Boolean(
        serversSection.element.compareDocumentPosition(addButton.element) &
        Node.DOCUMENT_POSITION_CONTAINED_BY
      )
    ).toBe(true);
    expect(wrapper.find('[data-test="mcp-server-row-github"]').text()).toContain("GitHub");
    expect(wrapper.find('[data-test="mcp-server-row-github"]').text()).toContain("running");
    expect(wrapper.find('[data-test="mcp-server-row-github"]').text()).toContain("5 tools");
    expect(wrapper.find('[data-test="mcp-trust-github"]').text()).toContain("Trust");
    expect(wrapper.find('[data-test="mcp-server-row-builtin-docs"]').text()).toContain(
      "connection refused"
    );
    expect(wrapper.find('[data-test="mcp-edit-github"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="mcp-delete-github"]').exists()).toBe(true);
  });

  it("filters servers and embeds Marketplace without Installed tab in a secondary sub-tab", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="mcp-search"]').setValue("docs");
    expect(wrapper.find('[data-test="mcp-server-row-github"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="mcp-server-row-builtin-docs"]').exists()).toBe(true);

    await wrapper.find('[data-test="mcp-subtab-marketplace"]').trigger("click");
    expect(wrapper.find('[data-test="mcp-marketplace-embedded"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="tab-installed"]').exists()).toBe(false);
  });

  it("labels the config action as a folder opener and delegates to the MCP store", async () => {
    const wrapper = mountPane();
    await flushPromises();

    const openConfigButton = wrapper.find('[data-test="mcp-open-config"]');
    expect(openConfigButton.text()).toContain("Open config folder");

    await openConfigButton.trigger("click");
    await flushPromises();

    expect(mockedCommands.openMcpConfigFile).toHaveBeenCalledTimes(1);
  });

  it("does not show the config folder action as opening while settings are loading", async () => {
    let resolveSettings: (value: { status: "ok"; data: McpServerSettingsView[] }) => void;
    mockedCommands.listMcpServerSettings.mockReturnValueOnce(
      new Promise((resolve) => {
        resolveSettings = resolve;
      })
    );

    const wrapper = mountPane();
    await nextTick();

    const openConfigButton = wrapper.find<HTMLButtonElement>('[data-test="mcp-open-config"]');
    expect(openConfigButton.text()).toContain("Open config folder");
    expect(openConfigButton.element.disabled).toBe(false);

    resolveSettings!(ok([githubServer, readonlyServer]));
    await flushPromises();
  });

  it("disables the config folder action while the folder is opening", async () => {
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

    expect(openConfigButton.text()).toContain("Open config folder");
    expect(openConfigButton.element.disabled).toBe(false);
  });

  it("shows a page-level error when opening the config folder fails", async () => {
    mockedCommands.openMcpConfigFile.mockRejectedValueOnce(new Error("folder open denied"));
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="mcp-open-config"]').trigger("click");
    await flushPromises();

    expect(useMcpStore().settingsError).toBe(
      "Unable to open MCP config folder: folder open denied"
    );
    expect(wrapper.find('[data-test="mcp-page-error"]').text()).toContain(
      "Unable to open MCP config folder: folder open denied"
    );
  });

  it("opens the add server panel and saves manual stdio settings through the MCP store action", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="mcp-add-server-btn"]').trigger("click");
    expect(wrapper.find('[data-test="mcp-add-server-panel"]').exists()).toBe(true);

    await wrapper.find('[data-test="mcp-install-mode-manual"]').trigger("click");
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

  it("refreshes settings rows after runtime actions change server state", async () => {
    const stoppedGithubServer = {
      ...githubServer,
      runtime_status: "stopped",
      trusted: true,
      tool_count: 6
    };
    mockedCommands.listMcpServerSettings
      .mockResolvedValueOnce(ok([githubServer]))
      .mockResolvedValueOnce(ok([stoppedGithubServer]));

    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="mcp-start-stop-github"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("stop_mcp_server", { serverId: "github" });
    expect(mockedCommands.listMcpServerSettings).toHaveBeenCalledTimes(2);
    expect(wrapper.find('[data-test="mcp-server-row-github"]').text()).toContain("stopped");
    expect(wrapper.find('[data-test="mcp-server-row-github"]').text()).toContain("6 tools");
  });

  it("keeps edit disabled until transport details can be edited without data loss", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find<HTMLButtonElement>('[data-test="mcp-edit-github"]').element.disabled).toBe(
      true
    );
  });

  it("releases row busy state when runtime actions fail", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("runtime unavailable"));
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="mcp-start-stop-github"]').trigger("click");
    await flushPromises();

    expect(
      wrapper.find<HTMLButtonElement>('[data-test="mcp-start-stop-github"]').element.disabled
    ).toBe(false);
  });

  it("disables write actions for read-only backend rows", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(
      wrapper.find<HTMLButtonElement>('[data-test="mcp-edit-builtin-docs"]').element.disabled
    ).toBe(true);
    expect(
      wrapper.find<HTMLButtonElement>('[data-test="mcp-delete-builtin-docs"]').element.disabled
    ).toBe(true);
  });

  it("shows page-level errors from the MCP store", async () => {
    mockedCommands.listMcpServerSettings.mockRejectedValueOnce(new Error("settings unavailable"));
    const wrapper = mountPane();
    await flushPromises();

    expect(useMcpStore().settingsError).toBe("settings unavailable");
    expect(wrapper.find('[role="alert"]').text()).toContain("settings unavailable");
  });
});
