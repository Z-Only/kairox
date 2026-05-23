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

import sessionSectionSource from "./sidebar/SessionSection.vue?raw";
import projectSectionSource from "./sidebar/ProjectSection.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import { useSessionStore } from "@/stores/session";
import {
  installSidebarTestEnv,
  mockInvokeCommandResponses,
  mountSidebar
} from "./SessionsSidebar.test-utils";

installSidebarTestEnv();

describe("SessionsSidebar", () => {
  it("renders session titles", async () => {
    // mountSidebar() activates a fresh Pinia internally; we mutate the
    // session store *after* mount and then re-render so the active Pinia
    // instance the component sees is the same one we modify.
    const { wrapper } = await mountSidebar();
    const session = useSessionStore();
    session.sessions = [
      { id: "s1", title: "Chat about Rust", profile: "fast" } as never,
      { id: "s2", title: "Debug session", profile: "slow" } as never
    ];
    await flushPromises();
    expect(wrapper.text()).toContain("Chat about Rust");
    expect(wrapper.text()).toContain("Debug session");
  });

  it("shows empty hint when no sessions", async () => {
    const { wrapper } = await mountSidebar();
    // The empty-state component renders the description text we passed in.
    const empty = wrapper.find('[data-test="sessions-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("No sessions yet");
    expect(empty.classes()).toContain("kx-empty-state");
    expect(empty.classes()).toContain("kx-empty-state--inline");
  });

  it("removes the redundant sidebar header and keeps the new session action in the sessions section", async () => {
    const { wrapper } = await mountSidebar();

    expect(wrapper.find('[data-test="sessions-sidebar-header"]').exists()).toBe(false);
    expect(
      wrapper.find('[data-test="sessions-section"] [data-test="new-session-btn"]').exists()
    ).toBe(true);
  });

  it("uses inline SVG icons rather than emoji action labels", () => {
    const sectionSources = [sessionSectionSource, projectSectionSource].join("\n");
    expect(sectionSources).toContain("<svg");
    expect(sectionSources).not.toContain("✏️");
    expect(sectionSources).not.toContain("🗑️");
  });

  it("P2-S2-sidebar-landmark-name: gives the sessions sidebar a unique accessible name", async () => {
    const { wrapper } = await mountSidebar();

    expect(wrapper.find('[data-test="sessions-sidebar"]').attributes("aria-label")).toBe(
      "Sessions"
    );
  });

  it("P2-S2-session-action-aria-labels: gives icon-only session actions stable accessible names", async () => {
    const { wrapper } = await mountSidebar();
    const session = useSessionStore();
    session.sessions = [{ id: "s1", title: "Session 1", profile: "fast" } as never];
    await flushPromises();

    expect(wrapper.find('[data-test="session-rename-btn"]').attributes("aria-label")).toBe(
      "Rename"
    );
    expect(wrapper.find('[data-test="session-archive-btn"]').attributes("aria-label")).toBe(
      "Archive"
    );
  });

  it("P2-S2-new-session-contrast: uses kx-icon-button for the new session action", () => {
    expectSourceMigration(sessionSectionSource, {
      required: ['data-test="new-session-btn"', "<KxIconButton"]
    });
  });

  it("renders project navigation above regular sessions by default", async () => {
    mockInvokeCommandResponses({
      list_projects: [
        {
          project_id: "project-1",
          display_name: "Demo",
          root_path: "/tmp/demo",
          removed_at: null,
          sort_order: 0,
          expanded: false
        }
      ]
    });
    const { wrapper } = await mountSidebar();
    const session = useSessionStore();

    session.sessions = [{ id: "s1", title: "Regular session", profile: "fast" } as never];
    await flushPromises();

    const projectSection = wrapper.find('[data-test="projects-section"]');
    const sessionsSection = wrapper.find('[data-test="sessions-section"]');

    expect(projectSection.exists()).toBe(true);
    expect(sessionsSection.exists()).toBe(true);
    expect(projectSection.text()).toContain("Demo");
    expect(projectSection.element.compareDocumentPosition(sessionsSection.element)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
  });
});
