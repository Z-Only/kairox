import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import FileMentionPalette from "./FileMentionPalette.vue";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@/generated/commands", () => ({
  commands: {
    listWorkspaceFiles: vi.fn()
  }
}));

import { commands } from "@/generated/commands";

const mockedCommands = vi.mocked(commands);
const defaultFiles = [
  "apps/agent-gui/src/components/ChatComposer.vue",
  "apps/agent-gui/src/components/FileMentionPalette.vue"
];

function setWorkspaceFiles(paths: string[]) {
  mockedCommands.listWorkspaceFiles.mockResolvedValue({
    status: "ok",
    data: { paths }
  });
}

async function mountOpenPalette() {
  const { wrapper } = mountWithPlugins(FileMentionPalette, {
    mount: {
      props: {
        visible: false,
        filterText: "",
        workspacePath: "/workspace"
      }
    }
  });

  await wrapper.setProps({ visible: true });
  await flushPromises();

  return wrapper;
}

beforeEach(() => {
  vi.clearAllMocks();
  setWorkspaceFiles(defaultFiles);
});

describe("FileMentionPalette", () => {
  it("keeps the palette visible with an empty state when no files match", async () => {
    const { wrapper } = mountWithPlugins(FileMentionPalette, {
      mount: {
        props: {
          visible: false,
          filterText: "",
          workspacePath: "/workspace"
        }
      }
    });

    await wrapper.setProps({ visible: true, filterText: "definitely-no-match" });
    await flushPromises();

    expect(wrapper.find('[data-test="file-mention-palette"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="file-mention-palette"]').classes()).toContain(
      "kx-popover-content"
    );
    expect(wrapper.find('[data-test="file-mention-palette"]').classes()).toContain(
      "kx-popover-content--palette"
    );
    expect(wrapper.find(".file-mention-palette__header").classes()).toContain(
      "kx-popover-panel__header"
    );
    expect(wrapper.find('[data-test="file-mention-empty"]').text()).toBe("No matching files");
    expect(wrapper.find('[data-test="file-mention-empty"]').classes()).toContain(
      "kx-popover-empty"
    );
    expect(wrapper.find('[data-test="file-mention-empty"]').classes()).toContain("kx-empty-state");
    expect(wrapper.find('[data-test="file-mention-empty"]').classes()).toContain(
      "kx-empty-state--popover"
    );
  });

  it("explains that file mentions need a project workspace", async () => {
    const { wrapper } = mountWithPlugins(FileMentionPalette, {
      mount: {
        props: {
          visible: true,
          filterText: "",
          workspacePath: ""
        }
      }
    });
    await flushPromises();

    expect(wrapper.find('[data-test="file-mention-palette"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="file-mention-empty"]').text()).toBe(
      "Open a project session to mention workspace files."
    );
  });

  it("uses shared option classes for file results", async () => {
    const { wrapper } = mountWithPlugins(FileMentionPalette, {
      mount: {
        props: {
          visible: false,
          filterText: "",
          workspacePath: "/workspace"
        }
      }
    });

    await wrapper.setProps({ visible: true });
    await flushPromises();

    const item = wrapper.find('[data-test="mention-file-item"]');
    expect(item.exists()).toBe(true);
    expect(item.classes()).toContain("kx-popover-option");
    expect(item.classes()).toContain("kx-popover-option--selected");
  });

  it("moves the selected file with ArrowDown", async () => {
    const wrapper = await mountOpenPalette();

    await wrapper.trigger("keydown", { key: "ArrowDown" });
    await wrapper.vm.$nextTick();

    const items = wrapper.findAll('[data-test="mention-file-item"]');
    expect(items[0].classes()).not.toContain("file-mention-palette__item--selected");
    expect(items[1].classes()).toContain("file-mention-palette__item--selected");
    expect(items[1].classes()).toContain("kx-popover-option--selected");
  });

  it("emits the selected file path on Enter", async () => {
    const wrapper = await mountOpenPalette();

    await wrapper.trigger("keydown", { key: "ArrowDown" });
    await wrapper.trigger("keydown", { key: "Enter" });

    expect(wrapper.emitted("select-file")).toEqual([[defaultFiles[1]]]);
  });

  it("emits close on Escape", async () => {
    const wrapper = await mountOpenPalette();

    await wrapper.trigger("keydown", { key: "Escape" });

    expect(wrapper.emitted("close")).toEqual([[]]);
  });

  it("does not emit a selection event on Enter when the file list is empty", async () => {
    setWorkspaceFiles([]);
    const wrapper = await mountOpenPalette();

    await wrapper.trigger("keydown", { key: "Enter" });

    expect(wrapper.findAll('[data-test="mention-file-item"]')).toHaveLength(0);
    expect(wrapper.emitted("select-file")).toBeUndefined();
  });
});
