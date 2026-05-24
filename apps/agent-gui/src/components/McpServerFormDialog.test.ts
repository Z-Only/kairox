import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { McpServerSettingsView } from "@/generated/commands";
import { useMcpStore } from "@/stores/mcp";
import { mountWithPlugins } from "@/test-utils/mount";
import mcpServerFormDialogSource from "./McpServerFormDialog.vue?raw";
import McpServerFormDialog from "./McpServerFormDialog.vue";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = vi.fn();
  HTMLDialogElement.prototype.close = vi.fn();
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

function savedServer(overrides: Partial<McpServerSettingsView> = {}): McpServerSettingsView {
  return {
    id: "filesystem",
    name: "filesystem",
    transport: "stdio",
    enabled: true,
    runtime_status: "stopped",
    trusted: false,
    tool_count: 0,
    last_error: null,
    writable: true,
    config_path: "/tmp/mcp.toml",
    description: null,
    source: "user_config",
    ...overrides
  };
}

function mountDialog() {
  return mountWithPlugins(McpServerFormDialog, {
    reusePinia: true,
    mount: {
      props: {
        open: true,
        mode: "manual"
      }
    }
  }).wrapper;
}

describe("McpServerFormDialog", () => {
  it("uses shared form fields and controls instead of local input chrome", () => {
    expectSourceMigration(mcpServerFormDialogSource, {
      required: ["KxFormField", "KxInput"],
      forbidden: ["kx-form-control", ".form label + input", ".form label + input:focus"]
    });
  });

  it("saves manual stdio settings with trimmed fields and parsed args", async () => {
    const mcp = useMcpStore();
    const saveServerSettings = vi
      .spyOn(mcp, "saveServerSettings")
      .mockResolvedValue(savedServer({ id: "filesystem", name: "filesystem" }));
    const wrapper = mountDialog();

    await wrapper.find('[data-test="mcp-form-name"]').setValue("  filesystem  ");
    await wrapper.find('[data-test="mcp-form-description"]').setValue("  Local files  ");
    await wrapper.find('[data-test="mcp-form-command"]').setValue("  npx  ");
    await wrapper
      .find('[data-test="mcp-form-args"]')
      .setValue("  -y   @modelcontextprotocol/server-filesystem   /tmp/project  ");
    await wrapper.find('[data-test="mcp-save-button"]').trigger("click");
    await flushPromises();

    expect(saveServerSettings).toHaveBeenCalledWith({
      name: "filesystem",
      transport: {
        transport: "stdio",
        command: "npx",
        args: ["-y", "@modelcontextprotocol/server-filesystem", "/tmp/project"],
        env: {}
      },
      enabled: true,
      description: "Local files"
    });
    expect(wrapper.emitted("close")).toHaveLength(1);
  });

  it("does not emit close when saving returns null", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "saveServerSettings").mockResolvedValue(null);
    const wrapper = mountDialog();

    await wrapper.find('[data-test="mcp-form-name"]').setValue("filesystem");
    await wrapper.find('[data-test="mcp-form-command"]').setValue("npx");
    await wrapper.find('[data-test="mcp-save-button"]').trigger("click");
    await flushPromises();

    expect(wrapper.emitted("close")).toBeUndefined();
  });
});
