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

  describe("highlightSegments", () => {
    it("returns the full path as a single non-match segment when filter is empty", async () => {
      const wrapper = await mountOpenPalette();
      const items = wrapper.findAll('[data-test="mention-file-item"]');
      expect(items.length).toBeGreaterThan(0);
      // With no filter, there should be no <mark> elements
      expect(items[0].findAll("mark")).toHaveLength(0);
      expect(items[0].text()).toContain("@");
    });

    it("highlights matching characters from the filter text", async () => {
      const { wrapper } = mountWithPlugins(FileMentionPalette, {
        mount: {
          props: {
            visible: false,
            filterText: "",
            workspacePath: "/workspace"
          }
        }
      });

      await wrapper.setProps({ visible: true, filterText: "Chat" });
      await flushPromises();

      const items = wrapper.findAll('[data-test="mention-file-item"]');
      expect(items.length).toBeGreaterThan(0);
      const marks = items[0].findAll("mark");
      expect(marks.length).toBeGreaterThan(0);
      const matchedText = marks.map((m) => m.text()).join("");
      expect(matchedText.toLowerCase()).toBe("chat");
    });
  });

  describe("showLoading", () => {
    it("shows loading state when workspace exists but files are not loaded yet", async () => {
      // Make listWorkspaceFiles hang (never resolve) to simulate loading state
      mockedCommands.listWorkspaceFiles.mockReturnValue(new Promise(() => {}));

      const { wrapper } = mountWithPlugins(FileMentionPalette, {
        mount: {
          props: {
            visible: true,
            filterText: "",
            workspacePath: "/workspace"
          }
        }
      });
      await wrapper.vm.$nextTick();

      expect(wrapper.find('[data-test="file-mention-loading"]').exists()).toBe(true);
    });
  });

  describe("arrow key bounds", () => {
    it("ArrowDown does not go past the last item", async () => {
      const wrapper = await mountOpenPalette();
      const items = wrapper.findAll('[data-test="mention-file-item"]');
      const lastIndex = items.length - 1;

      // Press ArrowDown more times than there are items
      for (let i = 0; i <= lastIndex + 2; i++) {
        await wrapper.trigger("keydown", { key: "ArrowDown" });
      }
      await wrapper.vm.$nextTick();

      const updatedItems = wrapper.findAll('[data-test="mention-file-item"]');
      expect(updatedItems[lastIndex].classes()).toContain("file-mention-palette__item--selected");
    });

    it("ArrowUp does not go below 0", async () => {
      const wrapper = await mountOpenPalette();

      // Press ArrowUp several times from index 0
      await wrapper.trigger("keydown", { key: "ArrowUp" });
      await wrapper.trigger("keydown", { key: "ArrowUp" });
      await wrapper.vm.$nextTick();

      const items = wrapper.findAll('[data-test="mention-file-item"]');
      expect(items[0].classes()).toContain("file-mention-palette__item--selected");
    });
  });

  it("emits select-file when clicking on a file item", async () => {
    const wrapper = await mountOpenPalette();

    const items = wrapper.findAll('[data-test="mention-file-item"]');
    expect(items.length).toBeGreaterThan(1);
    await items[1].trigger("click");

    expect(wrapper.emitted("select-file")).toEqual([[defaultFiles[1]]]);
  });

  describe("workspacePath watcher", () => {
    it("clears fileList and resets when workspace path becomes empty", async () => {
      const wrapper = await mountOpenPalette();

      // Should have items initially
      expect(wrapper.findAll('[data-test="mention-file-item"]').length).toBeGreaterThan(0);

      await wrapper.setProps({ workspacePath: "" });
      await flushPromises();

      // After clearing workspace, the empty message should appear
      expect(wrapper.find('[data-test="file-mention-empty"]').exists()).toBe(true);
    });

    it("reloads files when workspace path changes to a new value", async () => {
      const wrapper = await mountOpenPalette();
      mockedCommands.listWorkspaceFiles.mockClear();

      const newFiles = ["new-project/README.md"];
      mockedCommands.listWorkspaceFiles.mockResolvedValue({
        status: "ok",
        data: { paths: newFiles }
      });

      await wrapper.setProps({ workspacePath: "/new-workspace" });
      await flushPromises();

      expect(mockedCommands.listWorkspaceFiles).toHaveBeenCalledWith("/new-workspace");
      const items = wrapper.findAll('[data-test="mention-file-item"]');
      expect(items).toHaveLength(1);
      expect(items[0].text()).toContain("README.md");
    });
  });

  describe("visible watcher edge cases", () => {
    it("does not reload files when reopened and files are already loaded", async () => {
      const wrapper = await mountOpenPalette();

      // Files are loaded from the first open. Clear mock call count.
      mockedCommands.listWorkspaceFiles.mockClear();

      // Close and reopen
      await wrapper.setProps({ visible: false });
      await wrapper.setProps({ visible: true });
      await flushPromises();

      // Should not reload since filesLoaded is already true
      expect(mockedCommands.listWorkspaceFiles).not.toHaveBeenCalled();
    });

    it("clears files when visible becomes true but workspacePath is empty", async () => {
      const { wrapper } = mountWithPlugins(FileMentionPalette, {
        mount: {
          props: {
            visible: false,
            filterText: "",
            workspacePath: ""
          }
        }
      });

      await wrapper.setProps({ visible: true });
      await flushPromises();

      expect(wrapper.find('[data-test="file-mention-empty"]').exists()).toBe(true);
    });
  });

  describe("arrow keys on empty list", () => {
    it("ArrowDown does nothing when file list is empty", async () => {
      setWorkspaceFiles([]);
      const wrapper = await mountOpenPalette();

      // Should not throw
      await wrapper.trigger("keydown", { key: "ArrowDown" });
      await wrapper.vm.$nextTick();

      expect(wrapper.findAll('[data-test="mention-file-item"]')).toHaveLength(0);
    });

    it("ArrowUp does nothing when file list is empty", async () => {
      setWorkspaceFiles([]);
      const wrapper = await mountOpenPalette();

      await wrapper.trigger("keydown", { key: "ArrowUp" });
      await wrapper.vm.$nextTick();

      expect(wrapper.findAll('[data-test="mention-file-item"]')).toHaveLength(0);
    });
  });

  it("updates selectedIndex on mouseenter", async () => {
    const wrapper = await mountOpenPalette();

    const items = wrapper.findAll('[data-test="mention-file-item"]');
    expect(items.length).toBeGreaterThan(1);

    // Initially the first item is selected
    expect(items[0].classes()).toContain("file-mention-palette__item--selected");

    // Hover over the second item
    await items[1].trigger("mouseenter");
    await wrapper.vm.$nextTick();

    const updatedItems = wrapper.findAll('[data-test="mention-file-item"]');
    expect(updatedItems[0].classes()).not.toContain("file-mention-palette__item--selected");
    expect(updatedItems[1].classes()).toContain("file-mention-palette__item--selected");
  });
});
