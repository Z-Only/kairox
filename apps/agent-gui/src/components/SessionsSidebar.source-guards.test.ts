import { describe, it } from "vitest";
import sessionsSidebarSource from "./SessionsSidebar.vue?raw";
import sessionSectionSource from "./sidebar/SessionSection.vue?raw";
import projectSectionSource from "./sidebar/ProjectSection.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

describe("SessionsSidebar", () => {
  it("removes obsolete profile dialog and dropdown CSS", () => {
    expectSourceMigration(sessionsSidebarSource, {
      forbidden: [".new-session-dialog", ".profile-dropdown", ".profile-option", ".dialog-actions"]
    });
  });

  it("keeps row actions visually hidden until hover or keyboard focus", () => {
    expectSourceMigration(sessionsSidebarSource, {
      requiredPatterns: [
        /\.row-actions\s*\{[\s\S]*opacity:\s*0/,
        /\.session-item:hover\s+\.row-actions/,
        /\.project-row:hover\s+\.row-actions/,
        /:focus-within\s+\.row-actions/
      ]
    });
  });

  it("keeps project and regular session lists in independent scroll regions", () => {
    expectSourceMigration(projectSectionSource, {
      required: ['data-test="projects-scroll-region"']
    });
    expectSourceMigration(sessionSectionSource, {
      required: ['data-test="sessions-scroll-region"']
    });
    expectSourceMigration(sessionsSidebarSource, {
      requiredPatterns: [
        /\.sessions-sidebar \.sidebar-section\s*\{[\s\S]*max-height:/,
        /\.sessions-sidebar \.sidebar-section-scroll\s*\{[\s\S]*overflow-y:\s*auto/
      ]
    });
  });
});
