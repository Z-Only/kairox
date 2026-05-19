import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import ArchiveSettingsPane from "./ArchiveSettingsPane.vue";
import archiveSettingsPaneSource from "./ArchiveSettingsPane.vue?raw";
import { confirmDialogKey } from "@/composables/useConfirm";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@/generated/commands", () => ({
  commands: {
    restoreArchivedSession: vi.fn(),
    permanentlyDeleteSession: vi.fn()
  }
}));

import { invoke } from "@tauri-apps/api/core";
import { commands } from "@/generated/commands";

const mockedInvoke = vi.mocked(invoke);
const mockedCommands = vi.mocked(commands);

const archivedSession = {
  id: "ses_archived",
  title: "Archived task",
  profile: "default",
  permission_mode: null,
  project_id: "project_1",
  worktree_path: "/tmp/kairox-worktree",
  branch: "fix/archive",
  visibility: "archived"
};

function mountArchive(confirmMock = vi.fn().mockResolvedValue(true)) {
  return mountWithPlugins(ArchiveSettingsPane, {
    reusePinia: true,
    mount: {
      global: {
        provide: {
          [confirmDialogKey as symbol]: { confirm: confirmMock }
        }
      }
    }
  });
}

beforeEach(() => {
  vi.clearAllMocks();
  setActivePinia(createPinia());
  mockedInvoke.mockImplementation((command) => {
    if (command === "list_archived_sessions") {
      return Promise.resolve([archivedSession]);
    }
    return Promise.resolve([]);
  });
  mockedCommands.permanentlyDeleteSession.mockResolvedValue({ status: "ok", data: null });
  mockedCommands.restoreArchivedSession.mockResolvedValue({ status: "ok", data: null });
});

describe("ArchiveSettingsPane", () => {
  it("renders archived sessions with shared settings card list chrome", async () => {
    const { wrapper } = mountArchive();
    await flushPromises();

    expect(wrapper.find('[data-test="archive-list"]').classes()).toContain("settings-card-list");
    expect(wrapper.find('[data-test="archive-row-ses_archived"]').classes()).toContain(
      "settings-card-item"
    );
  });

  it("uses the app confirm dialog before permanently deleting an archived session", async () => {
    const confirmMock = vi.fn().mockResolvedValue(false);
    const { wrapper } = mountArchive(confirmMock);
    await flushPromises();

    await wrapper.find('[data-test="archive-delete-ses_archived"]').trigger("click");
    await flushPromises();

    expect(confirmMock).toHaveBeenCalledWith(
      expect.objectContaining({
        message: "Permanently delete this session? This cannot be undone."
      })
    );
    expect(mockedCommands.permanentlyDeleteSession).not.toHaveBeenCalled();
  });

  it("permanently deletes the session only after the app confirm dialog is accepted", async () => {
    const confirmMock = vi.fn().mockResolvedValue(true);
    const { wrapper } = mountArchive(confirmMock);
    await flushPromises();

    await wrapper.find('[data-test="archive-delete-ses_archived"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.permanentlyDeleteSession).toHaveBeenCalledWith("ses_archived");
  });

  it("uses the shared state block for an empty archive", async () => {
    mockedInvoke.mockImplementation((command) => {
      if (command === "list_archived_sessions") {
        return Promise.resolve([]);
      }
      return Promise.resolve([]);
    });

    const { wrapper } = mountArchive();
    await flushPromises();

    const empty = wrapper.find('[data-test="archive-empty-state"]');
    expect(empty.exists()).toBe(true);
    expect(empty.classes()).toContain("settings-state");
    expect(empty.classes()).toContain("kx-state-block--empty");
    expect(empty.text()).toContain("No archived sessions.");
  });

  it("does not keep local archive row card chrome after moving to SettingsCardItem", () => {
    expect(archiveSettingsPaneSource).toContain("SettingsCardList");
    expect(archiveSettingsPaneSource).toContain("SettingsCardItem");
    expect(archiveSettingsPaneSource).not.toContain('class="card archive-row"');
    expect(archiveSettingsPaneSource).not.toContain('class="card-body archive-row__body"');
    expect(archiveSettingsPaneSource).not.toContain(".archive-list {");
  });
});
