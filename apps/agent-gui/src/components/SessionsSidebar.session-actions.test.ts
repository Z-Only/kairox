import { describe, it, expect, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn()
}));
vi.mock("../composables/useTraceStore", () => ({
  applyTraceEvent: vi.fn(),
  clearTrace: vi.fn()
}));

import sidebarActionsSource from "@/composables/sidebar/useSidebarActions.ts?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import { useSessionStore } from "@/stores/session";
import { useWorkspaceUiStore } from "@/stores/workspaceUi";
import { installSidebarTestEnv, mockedInvoke, mountSidebar } from "./SessionsSidebar.test-utils";

installSidebarTestEnv();

describe("SessionsSidebar", () => {
  it("opens an ordinary placeholder chat without creating a backend session", async () => {
    const { wrapper, router } = await mountSidebar();
    const session = useSessionStore();
    session.currentSessionId = "ses_existing";
    const createSessionSpy = vi.spyOn(session, "createSession");

    await wrapper.find('[data-test="new-session-btn"]').trigger("click");
    await flushPromises();
    await router.isReady();

    expect(wrapper.find('[data-test="new-session-dialog"]').exists()).toBe(false);
    expect(createSessionSpy).not.toHaveBeenCalled();
    expect(session.currentSessionId).toBeNull();
    expect(session.composerDraftKey).toBe("new-session:ordinary");
    expect(router.currentRoute.value.name).toBe("workbench");
    expect(router.currentRoute.value.params.sessionId).toBeUndefined();
    expect(mockedInvoke).toHaveBeenCalledWith("refresh_config");
    expect(mockedInvoke).toHaveBeenCalledWith("get_profile_info");
  });

  it("uses Kairox icon buttons and title-backed truncation for regular session rows", async () => {
    const longTitle = "A very long regular session title that should remain discoverable";
    const { wrapper } = await mountSidebar();
    const session = useSessionStore();
    session.sessions = [{ id: "s1", title: longTitle, profile: "fast" } as never];
    await flushPromises();

    const sessionTitle = wrapper.find('[data-test="session-item"] .session-title');
    expect(sessionTitle.attributes("title")).toBe(longTitle);
    expect(sessionTitle.classes()).toContain("truncate");
    expect(wrapper.find('[data-test="session-rename-btn"]').classes()).toContain("kx-icon-button");
    expect(wrapper.find('[data-test="session-archive-btn"]').classes()).toContain("kx-icon-button");

    await wrapper.find('[data-test="session-rename-btn"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="session-rename-confirm"]').classes()).toContain(
      "kx-icon-button"
    );

    await wrapper.find('[data-test="session-rename-input"]').trigger("keydown.escape");
    await flushPromises();
    await wrapper.find('[data-test="session-archive-btn"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="session-archive-btn"]').trigger("click");
    await flushPromises();
    expect(mockedInvoke).toHaveBeenCalledWith("delete_session", { sessionId: "s1" });
  });

  it("requires a second click on the same session archive button before deleting", async () => {
    const { wrapper } = await mountSidebar();
    const session = useSessionStore();
    session.sessions = [{ id: "s1", title: "Session 1", profile: "fast" } as never];
    await flushPromises();

    await wrapper.find('[data-test="session-archive-btn"]').trigger("click");
    await flushPromises();
    expect(mockedInvoke).not.toHaveBeenCalledWith("delete_session", { sessionId: "s1" });

    await wrapper.find('[data-test="session-archive-btn"]').trigger("click");
    await flushPromises();
    expect(mockedInvoke).toHaveBeenCalledWith("delete_session", { sessionId: "s1" });
  });

  it("opens an ordinary draft after archiving the active regular session even when another session remains", async () => {
    const { wrapper, router } = await mountSidebar();
    const session = useSessionStore();
    const workspaceUi = useWorkspaceUiStore();
    session.sessions = [
      { id: "s1", title: "Session 1", profile: "fast" },
      { id: "s2", title: "Session 2", profile: "fast" }
    ] as never[];
    session.currentSessionId = "s2";
    session.projection.messages = [{ role: "user", content: "stale" }];
    workspaceUi.gitReviewContext = { sessionId: "s2", projectId: null };
    workspaceUi.gitReview = {
      branch: "main",
      changedFiles: ["stale.rs"],
      fileCount: 1,
      additions: 1,
      deletions: 0,
      staged: null,
      unstaged: null,
      untracked: null
    } as never;
    workspaceUi.gitReviewError = "stale error";
    await router.push("/workbench/s2");
    await router.isReady();
    await flushPromises();

    const archiveButtons = wrapper.findAll('[data-test="session-archive-btn"]');
    await archiveButtons[1].trigger("click");
    await flushPromises();
    await archiveButtons[1].trigger("click");
    await flushPromises();
    await router.isReady();

    expect(mockedInvoke).toHaveBeenCalledWith("delete_session", { sessionId: "s2" });
    expect(session.currentSessionId).toBeNull();
    expect(session.composerDraftKey).toBe("new-session:ordinary");
    expect(session.projection.messages).toEqual([]);
    expect(workspaceUi.gitReviewContext).toBeNull();
    expect(workspaceUi.gitReview).toBeNull();
    expect(workspaceUi.gitReviewError).toBeNull();
    expect(router.currentRoute.value.params.sessionId).toBeUndefined();
  });

  it("waits for session deletion before continuing after confirmation", () => {
    expectSourceMigration(sidebarActionsSource, {
      required: ["await session.deleteSession(sessionId)"],
      forbidden: ["void session.deleteSession"]
    });
  });

  it("audit anchors: exposes stable session lifecycle pilot selectors", async () => {
    const { wrapper } = await mountSidebar();
    const sessionStore = useSessionStore();
    vi.spyOn(sessionStore, "createSession");

    await wrapper.find('[data-test="new-session-btn"]').trigger("click");
    await flushPromises();
    expect(wrapper.find('[data-test="new-session-dialog"]').exists()).toBe(false);
    expect(sessionStore.createSession).not.toHaveBeenCalled();

    const session = useSessionStore();
    session.sessions = [{ id: "s1", title: "Session 1", profile: "fast" } as never];
    await flushPromises();

    const renameButton = wrapper.find('[data-test="session-rename-btn"]');
    expect(renameButton.exists()).toBe(true);
    await renameButton.trigger("click");
    await flushPromises();

    expect(wrapper.find(".kx-editable-label").exists()).toBe(true);
    const renameInput = wrapper.find('[data-test="session-rename-input"]');
    const renameConfirm = wrapper.find('[data-test="session-rename-confirm"]');
    expect(renameInput.exists()).toBe(true);
    expect(renameConfirm.exists()).toBe(true);
    expect(renameInput.attributes("data-test")).toBe("session-rename-input");
    expect(renameConfirm.attributes("data-test")).toBe("session-rename-confirm");
  });
});
