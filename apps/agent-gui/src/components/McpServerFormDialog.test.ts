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

  it("resets form fields when dialog reopens", async () => {
    const wrapper = mountDialog();

    // Fill in some data
    await wrapper.find('[data-test="mcp-form-name"]').setValue("my-server");
    await wrapper.find('[data-test="mcp-form-command"]').setValue("node");
    await wrapper.find('[data-test="mcp-form-args"]').setValue("index.js");
    await wrapper.find('[data-test="mcp-form-description"]').setValue("A test server");

    // Close and reopen
    await wrapper.setProps({ open: false });
    await wrapper.setProps({ open: true });
    await wrapper.vm.$nextTick();

    // Fields should be reset
    const nameInput = wrapper.find('[data-test="mcp-form-name"]');
    expect((nameInput.element as HTMLInputElement).value).toBe("");
    const commandInput = wrapper.find('[data-test="mcp-form-command"]');
    expect((commandInput.element as HTMLInputElement).value).toBe("");
    const descInput = wrapper.find('[data-test="mcp-form-description"]');
    expect((descInput.element as HTMLInputElement).value).toBe("");
  });

  it("parseArgs handles multiple spaces and empty strings", async () => {
    const mcp = useMcpStore();
    const saveServerSettings = vi.spyOn(mcp, "saveServerSettings").mockResolvedValue(savedServer());
    const wrapper = mountDialog();

    await wrapper.find('[data-test="mcp-form-name"]').setValue("test");
    await wrapper.find('[data-test="mcp-form-command"]').setValue("node");
    await wrapper.find('[data-test="mcp-form-args"]').setValue("   ");
    await wrapper.find('[data-test="mcp-save-button"]').trigger("click");
    await flushPromises();

    expect(saveServerSettings).toHaveBeenCalledWith(
      expect.objectContaining({
        transport: expect.objectContaining({
          args: []
        })
      })
    );
  });

  it("shows URL field when SSE transport is selected", async () => {
    const wrapper = mountDialog();

    // Click the SSE radio button
    await wrapper.find('[data-test="mcp-form-sse"]').setValue(true);
    await wrapper.vm.$nextTick();

    // URL field should be visible, command/args should be hidden
    expect(wrapper.find('[data-test="mcp-form-url"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="mcp-form-command"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="mcp-form-args"]').exists()).toBe(false);
  });

  it("shows git URL field and hides manual transport options in git mode", async () => {
    const { wrapper } = mountWithPlugins(McpServerFormDialog, {
      reusePinia: true,
      mount: {
        props: {
          open: true,
          mode: "git"
        }
      }
    });
    await wrapper.vm.$nextTick();

    // Git mode should show git URL field
    expect(wrapper.find('[data-test="mcp-form-git-url"]').exists()).toBe(true);
    // Transport radio buttons should not be visible
    expect(wrapper.find('[data-test="mcp-form-stdio"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="mcp-form-sse"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="mcp-form-streamable-http"]').exists()).toBe(false);
  });

  it("disables save button when serverName is empty or whitespace", async () => {
    const wrapper = mountDialog();

    // Initially empty
    const saveButton = wrapper.find('[data-test="mcp-save-button"]');
    expect(saveButton.attributes("disabled")).toBeDefined();

    // Set to whitespace only
    await wrapper.find('[data-test="mcp-form-name"]').setValue("   ");
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="mcp-save-button"]').attributes("disabled")).toBeDefined();

    // Set to a valid name
    await wrapper.find('[data-test="mcp-form-name"]').setValue("my-server");
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="mcp-save-button"]').attributes("disabled")).toBeUndefined();
  });

  it("saves SSE transport with correct payload", async () => {
    const mcp = useMcpStore();
    const saveServerSettings = vi
      .spyOn(mcp, "saveServerSettings")
      .mockResolvedValue(savedServer({ transport: "sse" }));
    const wrapper = mountDialog();

    await wrapper.find('[data-test="mcp-form-name"]').setValue("sse-server");
    await wrapper.find('[data-test="mcp-form-sse"]').setValue(true);
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="mcp-form-url"]').setValue("http://localhost:3000/sse");
    await wrapper.find('[data-test="mcp-save-button"]').trigger("click");
    await flushPromises();

    expect(saveServerSettings).toHaveBeenCalledWith({
      name: "sse-server",
      transport: {
        transport: "sse",
        url: "http://localhost:3000/sse",
        headers: {}
      },
      enabled: true,
      description: null
    });
  });

  it("saves streamable_http transport with correct payload", async () => {
    const mcp = useMcpStore();
    const saveServerSettings = vi
      .spyOn(mcp, "saveServerSettings")
      .mockResolvedValue(savedServer({ transport: "streamable_http" }));
    const wrapper = mountDialog();

    await wrapper.find('[data-test="mcp-form-name"]').setValue("http-server");
    await wrapper.find('[data-test="mcp-form-streamable-http"]').setValue(true);
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="mcp-form-url"]').setValue("http://localhost:3000/mcp");
    await wrapper.find('[data-test="mcp-save-button"]').trigger("click");
    await flushPromises();

    expect(saveServerSettings).toHaveBeenCalledWith({
      name: "http-server",
      transport: {
        transport: "streamable_http",
        url: "http://localhost:3000/mcp",
        headers: {}
      },
      enabled: true,
      description: null
    });
  });

  it("sends description as null when empty after trim", async () => {
    const mcp = useMcpStore();
    const saveServerSettings = vi.spyOn(mcp, "saveServerSettings").mockResolvedValue(savedServer());
    const wrapper = mountDialog();

    await wrapper.find('[data-test="mcp-form-name"]').setValue("test-server");
    await wrapper.find('[data-test="mcp-form-description"]').setValue("   ");
    await wrapper.find('[data-test="mcp-form-command"]').setValue("node");
    await wrapper.find('[data-test="mcp-save-button"]').trigger("click");
    await flushPromises();

    expect(saveServerSettings).toHaveBeenCalledWith(
      expect.objectContaining({
        description: null
      })
    );
  });

  it("does not save when serverName is empty", async () => {
    const mcp = useMcpStore();
    const saveServerSettings = vi.spyOn(mcp, "saveServerSettings");
    const wrapper = mountDialog();

    await wrapper.find('[data-test="mcp-form-command"]').setValue("node");
    await wrapper.find('[data-test="mcp-save-button"]').trigger("click");
    await flushPromises();

    expect(saveServerSettings).not.toHaveBeenCalled();
  });
});
