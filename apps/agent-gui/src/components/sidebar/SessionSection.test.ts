import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { enableAutoUnmount } from "@vue/test-utils";
import { ref, type Ref } from "vue";
import type { SidebarRenameController } from "@/composables/sidebar/useSidebarRename";
import type { SessionInfoResponse } from "@/types";
import SessionSection from "./SessionSection.vue";
import { mountWithPlugins } from "@/test-utils/mount";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeSession(overrides: Partial<SessionInfoResponse> = {}): SessionInfoResponse {
  return {
    id: "sess-1",
    title: "Default Session",
    profile: "default",
    approval_policy: null,
    sandbox_policy: null,
    project_id: null,
    worktree_path: null,
    branch: null,
    visibility: null,
    deleted_at: null,
    ...overrides
  };
}

function makeRenameController(
  overrides: Partial<SidebarRenameController> = {}
): SidebarRenameController {
  return {
    editingId: ref(null) as Ref<string | null>,
    title: ref(""),
    input: ref(null),
    start: vi.fn(),
    bindInput: vi.fn(),
    confirm: vi.fn(),
    cancel: vi.fn(),
    ...overrides
  };
}

interface MountOptions {
  sessions?: SessionInfoResponse[];
  activeSessionId?: string | null;
  pendingDeleteSessionId?: string | null;
  rename?: SidebarRenameController;
  createSession?: () => Promise<void> | void;
  switchToSession?: (sessionId: string) => Promise<void> | void;
  requestDeleteSession?: (sessionId: string) => Promise<void> | void;
}

function mountSessionSection(opts: MountOptions = {}) {
  const rename = opts.rename ?? makeRenameController();

  const { wrapper, router } = mountWithPlugins(SessionSection, {
    initialRoute: "/workbench",
    mount: {
      props: {
        sessions: opts.sessions ?? [],
        activeSessionId: opts.activeSessionId ?? null,
        pendingDeleteSessionId: opts.pendingDeleteSessionId ?? null,
        rename,
        createSession: opts.createSession ?? vi.fn(),
        switchToSession: opts.switchToSession ?? vi.fn(),
        requestDeleteSession: opts.requestDeleteSession ?? vi.fn()
      } as Record<string, unknown>,
      global: {
        stubs: { Teleport: true }
      }
    }
  });
  return { wrapper, router, rename };
}

// ---------------------------------------------------------------------------
// Test environment
// ---------------------------------------------------------------------------

enableAutoUnmount(afterEach);
afterEach(() => {
  document.body.innerHTML = "";
});
beforeEach(() => {
  vi.clearAllMocks();
});

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe("SessionSection", () => {
  // ---- Rendering ----

  describe("rendering", () => {
    it("renders the sessions section container", () => {
      const { wrapper } = mountSessionSection();
      expect(wrapper.find('[data-test="sessions-section"]').exists()).toBe(true);
    });

    it("renders the section heading", () => {
      const { wrapper } = mountSessionSection();
      expect(wrapper.find(".section-heading h3").exists()).toBe(true);
    });

    it("renders the new-session button", () => {
      const { wrapper } = mountSessionSection();
      expect(wrapper.find('[data-test="new-session-btn"]').exists()).toBe(true);
    });

    it("renders session items when sessions are provided", () => {
      const sessions = [
        makeSession({ id: "s1", title: "Alpha" }),
        makeSession({ id: "s2", title: "Beta" })
      ];
      const { wrapper } = mountSessionSection({ sessions });

      const items = wrapper.findAll('[data-test="session-item"]');
      expect(items).toHaveLength(2);
      expect(items[0].text()).toContain("Alpha");
      expect(items[1].text()).toContain("Beta");
    });

    it("shows empty state when no sessions exist", () => {
      const { wrapper } = mountSessionSection({ sessions: [] });
      expect(wrapper.find('[data-test="sessions-empty"]').exists()).toBe(true);
      expect(wrapper.findAll('[data-test="session-item"]')).toHaveLength(0);
    });

    it("shows session title with title attribute", () => {
      const sessions = [makeSession({ id: "s1", title: "My Task" })];
      const { wrapper } = mountSessionSection({ sessions });

      const titleEl = wrapper.find(".session-title");
      expect(titleEl.text()).toBe("My Task");
      expect(titleEl.attributes("title")).toBe("My Task");
    });
  });

  // ---- Active session highlighting ----

  describe("active session", () => {
    it("applies active class to the matching session", () => {
      const sessions = [makeSession({ id: "s1" }), makeSession({ id: "s2" })];
      const { wrapper } = mountSessionSection({ sessions, activeSessionId: "s1" });

      const items = wrapper.findAll('[data-test="session-item"]');
      expect(items[0].classes()).toContain("active");
      expect(items[1].classes()).not.toContain("active");
    });

    it("does not apply active class when no session is active", () => {
      const sessions = [makeSession({ id: "s1" })];
      const { wrapper } = mountSessionSection({ sessions, activeSessionId: null });

      const item = wrapper.find('[data-test="session-item"]');
      expect(item.classes()).not.toContain("active");
    });
  });

  // ---- Create session ----

  describe("create session", () => {
    it("calls createSession when new-session button is clicked", async () => {
      const createSession = vi.fn();
      const { wrapper } = mountSessionSection({ createSession });

      await wrapper.find('[data-test="new-session-btn"]').trigger("click");
      expect(createSession).toHaveBeenCalledOnce();
    });
  });

  // ---- Switch session ----

  describe("switch session", () => {
    it("calls switchToSession when a session item is clicked", async () => {
      const switchToSession = vi.fn();
      const sessions = [makeSession({ id: "s1" })];
      const { wrapper } = mountSessionSection({ sessions, switchToSession });

      await wrapper.find('[data-test="session-item"]').trigger("click");
      expect(switchToSession).toHaveBeenCalledWith("s1");
    });

    it("does not call switchToSession when rename is active for that item", async () => {
      const switchToSession = vi.fn();
      const sessions = [makeSession({ id: "s1" })];
      const rename = makeRenameController({
        editingId: ref("s1") as Ref<string | null>
      });
      const { wrapper } = mountSessionSection({ sessions, switchToSession, rename });

      await wrapper.find('[data-test="session-item"]').trigger("click");
      expect(switchToSession).not.toHaveBeenCalled();
    });
  });

  // ---- Rename session ----

  describe("rename session", () => {
    it("shows editable label when session is being renamed", () => {
      const sessions = [makeSession({ id: "s1", title: "Old Title" })];
      const rename = makeRenameController({
        editingId: ref("s1") as Ref<string | null>,
        title: ref("Old Title")
      });
      const { wrapper } = mountSessionSection({ sessions, rename });

      expect(wrapper.find('[data-test="session-rename-input"]').exists()).toBe(true);
      // Session title span should be hidden during rename
      expect(wrapper.find(".session-title").exists()).toBe(false);
    });

    it("shows session title when not being renamed", () => {
      const sessions = [makeSession({ id: "s1", title: "My Title" })];
      const { wrapper } = mountSessionSection({ sessions });

      expect(wrapper.find(".session-title").exists()).toBe(true);
      expect(wrapper.find('[data-test="session-rename-input"]').exists()).toBe(false);
    });

    it("starts rename when rename button is clicked", async () => {
      const sessions = [makeSession({ id: "s1", title: "TaskName" })];
      const rename = makeRenameController();
      const { wrapper } = mountSessionSection({ sessions, rename });

      await wrapper.find('[data-test="session-rename-btn"]').trigger("click");
      expect(rename.start).toHaveBeenCalledWith("s1", "TaskName");
    });
  });

  // ---- Archive / delete session ----

  describe("archive session", () => {
    it("shows archive button for each session", () => {
      const sessions = [makeSession({ id: "s1" })];
      const { wrapper } = mountSessionSection({ sessions });

      expect(wrapper.find('[data-test="session-archive-btn"]').exists()).toBe(true);
    });

    it("shows confirm style when pending delete matches session", () => {
      const sessions = [makeSession({ id: "s1" })];
      const { wrapper } = mountSessionSection({
        sessions,
        pendingDeleteSessionId: "s1"
      });

      const archiveBtn = wrapper.find('[data-test="session-archive-btn"]');
      expect(archiveBtn.classes()).toContain("confirm-action");
    });

    it("does not show confirm style when pending delete does not match", () => {
      const sessions = [makeSession({ id: "s1" })];
      const { wrapper } = mountSessionSection({
        sessions,
        pendingDeleteSessionId: "other-id"
      });

      const archiveBtn = wrapper.find('[data-test="session-archive-btn"]');
      expect(archiveBtn.classes()).not.toContain("confirm-action");
    });

    it("calls requestDeleteSession when archive button is clicked", async () => {
      const requestDeleteSession = vi.fn();
      const sessions = [makeSession({ id: "s1" })];
      const { wrapper } = mountSessionSection({ sessions, requestDeleteSession });

      await wrapper.find('[data-test="session-archive-btn"]').trigger("click");
      expect(requestDeleteSession).toHaveBeenCalledWith("s1");
    });

    it("shows checkmark icon when pending delete matches session", () => {
      const sessions = [makeSession({ id: "s1" })];
      const { wrapper } = mountSessionSection({
        sessions,
        pendingDeleteSessionId: "s1"
      });

      // When pending delete matches, the first svg (checkmark) is shown
      const archiveBtn = wrapper.find('[data-test="session-archive-btn"]');
      const svgs = archiveBtn.findAll("svg");
      expect(svgs.length).toBeGreaterThanOrEqual(1);
    });
  });

  // ---- Scroll region ----

  describe("scroll region", () => {
    it("renders the scroll region container", () => {
      const { wrapper } = mountSessionSection();
      expect(wrapper.find('[data-test="sessions-scroll-region"]').exists()).toBe(true);
    });
  });

  // ---- Multiple sessions ----

  describe("multiple sessions", () => {
    it("renders all sessions in order", () => {
      const sessions = [
        makeSession({ id: "s1", title: "First" }),
        makeSession({ id: "s2", title: "Second" }),
        makeSession({ id: "s3", title: "Third" })
      ];
      const { wrapper } = mountSessionSection({ sessions });

      const items = wrapper.findAll('[data-test="session-item"]');
      expect(items).toHaveLength(3);
      expect(items[0].text()).toContain("First");
      expect(items[1].text()).toContain("Second");
      expect(items[2].text()).toContain("Third");
    });

    it("only highlights the active session among many", () => {
      const sessions = [
        makeSession({ id: "s1", title: "First" }),
        makeSession({ id: "s2", title: "Second" }),
        makeSession({ id: "s3", title: "Third" })
      ];
      const { wrapper } = mountSessionSection({ sessions, activeSessionId: "s2" });

      const items = wrapper.findAll('[data-test="session-item"]');
      expect(items[0].classes()).not.toContain("active");
      expect(items[1].classes()).toContain("active");
      expect(items[2].classes()).not.toContain("active");
    });
  });
});
