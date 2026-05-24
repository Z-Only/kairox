import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import ArchiveSettingsPane from "./ArchiveSettingsPane.vue";
import archiveSettingsPaneSource from "./ArchiveSettingsPane.vue?raw";
import { confirmDialogKey } from "@/composables/useConfirm";
import { useProjectStore } from "@/stores/project";
import { mountWithPlugins } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

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
  deleted_at: "2026-01-02T03:04:05Z",
  visibility: "archived"
};

const docsArchivedSession = {
  id: "ses_docs",
  title: "Docs cleanup",
  profile: "review",
  permission_mode: null,
  project_id: "project_docs",
  worktree_path: "/tmp/kairox-docs-worktree",
  branch: "docs/readme",
  deleted_at: "2026-01-03T04:05:06Z",
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
  useProjectStore().projects = [
    {
      projectId: "project_1",
      displayName: "Core Project",
      rootPath: "/tmp/kairox-worktree",
      removedAt: null,
      sortOrder: 0,
      expanded: true,
      pathExists: true
    },
    {
      projectId: "project_docs",
      displayName: "Docs Project",
      rootPath: "/tmp/kairox-docs-worktree",
      removedAt: null,
      sortOrder: 1,
      expanded: true,
      pathExists: true
    }
  ];
  mockedInvoke.mockImplementation((command) => {
    if (command === "list_archived_sessions") {
      return Promise.resolve([archivedSession, docsArchivedSession]);
    }
    return Promise.resolve([]);
  });
  mockedCommands.permanentlyDeleteSession.mockResolvedValue({ status: "ok", data: null });
  mockedCommands.restoreArchivedSession.mockResolvedValue({ status: "ok", data: null });
});

describe("ArchiveSettingsPane", () => {
  it("renders archived session search controls", async () => {
    const { wrapper } = mountArchive();
    await flushPromises();

    expect(wrapper.find('[data-test="archive-filters"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="archive-search-input"]').exists()).toBe(true);
  });

  it("renders archived sessions with shared settings card list chrome", async () => {
    const { wrapper } = mountArchive();
    await flushPromises();

    expect(wrapper.find('[data-test="archive-list"]').classes()).toContain("settings-card-list");
    expect(wrapper.find('[data-test="archive-list"]').classes()).toContain(
      "settings-card-list--auto-columns"
    );
    expect(wrapper.find('[data-test="archive-row-ses_archived"]').classes()).toContain(
      "settings-card-item"
    );
  });

  it("filters archived sessions by search text", async () => {
    const { wrapper } = mountArchive();
    await flushPromises();

    await wrapper.find('[data-test="archive-search-input"]').setValue("docs");

    expect(wrapper.find('[data-test="archive-row-ses_docs"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="archive-row-ses_archived"]').exists()).toBe(false);
  });

  it("matches archived session search against metadata", async () => {
    const { wrapper } = mountArchive();
    await flushPromises();

    await wrapper.find('[data-test="archive-search-input"]').setValue("core project");

    expect(wrapper.find('[data-test="archive-row-ses_archived"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="archive-row-ses_docs"]').exists()).toBe(false);

    await wrapper.find('[data-test="archive-search-input"]').setValue("docs/readme");
    expect(wrapper.find('[data-test="archive-row-ses_docs"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="archive-row-ses_archived"]').exists()).toBe(false);
  });

  it("shows the archived timestamp for each archived session", async () => {
    const { wrapper } = mountArchive();
    await flushPromises();

    const archivedAt = wrapper.find('[data-test="archive-time-ses_archived"]');
    expect(archivedAt.exists()).toBe(true);
    expect(archivedAt.attributes("datetime")).toBe("2026-01-02T03:04:05Z");
    expect(archivedAt.text()).toContain("Archived");
    expect(archivedAt.text()).toContain("2026");
  });

  it("shows a filtered empty state without replacing the genuine empty archive state", async () => {
    const { wrapper } = mountArchive();
    await flushPromises();

    await wrapper.find('[data-test="archive-search-input"]').setValue("does-not-exist");
    await flushPromises();

    const empty = wrapper.find('[data-test="archive-filter-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.classes()).toContain("settings-state");
    expect(empty.classes()).toContain("kx-state-block--empty");
    expect(empty.text()).toContain("No archived sessions match your search.");
    expect(wrapper.find('[data-test="archive-empty-state"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="archive-list"]').exists()).toBe(false);
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
    expectSourceMigration(archiveSettingsPaneSource, {
      required: [
        "SettingsCardList",
        "SettingsCardItem",
        "SettingsFilterBar",
        "SettingsItemSummary",
        "SettingsItemMeta"
      ],
      forbidden: [
        'class="card archive-row"',
        'class="card-body archive-row__body"',
        ".archive-list {",
        ".archive-row__meta"
      ]
    });
  });

  it("uses SettingsStatusTag for archive stats instead of direct tag markup", () => {
    expectSourceMigration(archiveSettingsPaneSource, {
      required: ["SettingsStatusTag"],
      forbidden: ['<span class="tag">']
    });
  });

  it("does not keep archive pane aria chrome inline in the component source", () => {
    expectSourceMigration(archiveSettingsPaneSource, {
      forbidden: ['aria-label="Archive"', 'aria-label="Archived sessions"']
    });
  });
});
