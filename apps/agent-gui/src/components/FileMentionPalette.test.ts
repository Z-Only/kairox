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

beforeEach(() => {
  vi.clearAllMocks();
  mockedCommands.listWorkspaceFiles.mockResolvedValue({
    status: "ok",
    data: { paths: ["apps/agent-gui/src/components/ChatComposer.vue"] }
  });
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
    expect(wrapper.find('[data-test="file-mention-empty"]').text()).toBe("No matching files");
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
});
