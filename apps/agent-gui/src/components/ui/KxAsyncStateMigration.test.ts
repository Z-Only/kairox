import { describe, expect, it } from "vitest";

import settingsStateSource from "@/components/ui/SettingsState.vue?raw";
import accordionStateSource from "@/components/ui/KxAccordionState.vue?raw";
import memoryBrowserSource from "@/components/MemoryBrowser.vue?raw";
import traceTimelineSource from "@/components/TraceTimeline.vue?raw";
import taskStepsSource from "@/components/TaskSteps.vue?raw";
import permissionCenterSource from "@/components/PermissionCenter.vue?raw";
import installedListSource from "@/components/marketplace/InstalledList.vue?raw";
import skillDiscoverListSource from "@/components/skills/SkillDiscoverList.vue?raw";
import mcpResourceAccordionSource from "@/components/McpResourceAccordion.vue?raw";
import mcpPromptAccordionSource from "@/components/McpPromptAccordion.vue?raw";
import chatPanelSource from "@/components/ChatPanel.vue?raw";
import sessionsSidebarSource from "@/components/SessionsSidebar.vue?raw";
import sessionSectionSource from "@/components/sidebar/SessionSection.vue?raw";
import commandPaletteSource from "@/components/CommandPalette.vue?raw";
import fileMentionPaletteSource from "@/components/FileMentionPalette.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

const migratedSources = [
  ["SettingsState.vue", settingsStateSource],
  ["KxAccordionState.vue", accordionStateSource],
  ["MemoryBrowser.vue", memoryBrowserSource],
  ["TraceTimeline.vue", traceTimelineSource],
  ["TaskSteps.vue", taskStepsSource],
  ["PermissionCenter.vue", permissionCenterSource],
  ["InstalledList.vue", installedListSource],
  ["SkillDiscoverList.vue", skillDiscoverListSource],
  ["McpResourceAccordion.vue", mcpResourceAccordionSource],
  ["McpPromptAccordion.vue", mcpPromptAccordionSource],
  ["ChatPanel.vue", chatPanelSource],
  ["SessionsSidebar.vue", sessionsSidebarSource],
  ["SessionSection.vue", sessionSectionSource],
  ["CommandPalette.vue", commandPaletteSource],
  ["FileMentionPalette.vue", fileMentionPaletteSource]
] as const;

describe("Kx async state migration", () => {
  it.each(migratedSources)(
    "%s routes empty/loading/error states through Kx state primitives",
    (_, source) => {
      expect(source).toMatch(/Kx(AsyncState|EmptyState)|SettingsState|KxAccordionState/);
    }
  );

  it("removes old local memory and task empty-state CSS", () => {
    expectSourceMigration(memoryBrowserSource, {
      forbidden: [".memory-empty {", ".empty-state {"]
    });
    expectSourceMigration(traceTimelineSource, { forbidden: [".empty-hint {"] });
    expectSourceMigration(taskStepsSource, { forbidden: [".empty-hint {"] });
    expectSourceMigration(permissionCenterSource, { forbidden: [".empty-state {"] });
    expectSourceMigration(skillDiscoverListSource, {
      forbidden: [".catalog-state {", ".spinner {"]
    });
    expectSourceMigration(chatPanelSource, { forbidden: [".empty-state {"] });
    expectSourceMigration(sessionsSidebarSource, {
      forbidden: [".sessions-empty-state {", ".empty-hint {"]
    });
    expectSourceMigration(sessionSectionSource, {
      forbidden: ['class="empty-state empty-hint"']
    });
    expectSourceMigration(fileMentionPaletteSource, {
      forbidden: ['class="kx-popover-empty']
    });
  });
});
