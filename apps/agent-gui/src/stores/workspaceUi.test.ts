import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { useWorkspaceUiStore } from "@/stores/workspaceUi";

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("useWorkspaceUiStore", () => {
  it("starts with the default section order and a closed archive", () => {
    const store = useWorkspaceUiStore();
    expect(store.sectionOrder).toEqual(["projects", "sessions"]);
    expect(store.archiveOpen).toBe(false);
  });

  it("moveSectionUp returns early when the section is already at the top", () => {
    const store = useWorkspaceUiStore();
    store.moveSectionUp("projects");
    expect(store.sectionOrder).toEqual(["projects", "sessions"]);
  });

  it("moveSectionUp returns early when the section is not in the order", () => {
    const store = useWorkspaceUiStore();
    store.moveSectionUp("unknown" as "projects");
    expect(store.sectionOrder).toEqual(["projects", "sessions"]);
  });

  it("moveSectionUp swaps the section with the one above it", () => {
    const store = useWorkspaceUiStore();
    store.moveSectionUp("sessions");
    expect(store.sectionOrder).toEqual(["sessions", "projects"]);
  });
});
