import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { nextTick } from "vue";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";
import { invoke } from "@tauri-apps/api/core";
import {
  commands,
  type EffectiveMcpServerView,
  type McpServerSettingsView
} from "@/generated/commands";
import { useMcpStore } from "@/stores/mcp";
import McpSettingsPane from "./McpSettingsPane.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("@/generated/commands", () => ({
  commands: {
    listMcpServerSettings: vi.fn(),
    upsertMcpServerSettings: vi.fn(),
    setMcpServerEnabled: vi.fn(),
    deleteMcpServerSettings: vi.fn(),
    openMcpConfigFile: vi.fn(),
    getEffectiveMcpServers: vi.fn()
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

function mountPane() {
  const mountOptions: MountWithPluginsOptions<typeof McpSettingsPane> = {
    reusePinia: true
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
    expect(wrapper.find('[data-test="mcp-add-server-dialog"]').exists()).toBe(false);
    expect(
      Boolean(
        serversSection.element.compareDocumentPosition(addButton.element) &
        Node.DOCUMENT_POSITION_CONTAINED_BY
      )
    ).toBe(true);
    expect(wrapper.find('[data-test="mcp-server-row-github"]').text()).toContain("GitHub");
    expect(wrapper.find('[data-test="mcp-server-row-github"]').text()).toContain("5 tools");
    expect(wrapper.find('[data-test="mcp-trust-github"]').text()).toContain("Trust");
    expect(wrapper.find('[data-test="mcp-server-row-builtin-docs"]').text()).toContain(
      "connection refused"
    );
    expect(wrapper.find('[data-test="mcp-delete-github"]').exists()).toBe(true);
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
    expect(wrapper.find('[data-test="mcp-page-error"]').text()).toContain(
      "Unable to open MCP config file: file open denied"
    );
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

  it("shows page-level errors from the MCP store", async () => {
    mockedCommands.listMcpServerSettings.mockRejectedValueOnce(new Error("settings unavailable"));
    const wrapper = mountPane();
    await flushPromises();

    expect(useMcpStore().settingsError).toBe("settings unavailable");
    expect(wrapper.find('[role="alert"]').text()).toContain("settings unavailable");
  });
});
