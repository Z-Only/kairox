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
  ["McpPromptAccordion.vue", mcpPromptAccordionSource]
] as const;

describe("Kx async state migration", () => {
  it.each(migratedSources)(
    "%s routes empty/loading/error states through Kx state primitives",
    (_, source) => {
      expect(source).toMatch(/Kx(AsyncState|EmptyState)|SettingsState|KxAccordionState/);
    }
  );

  it("removes old local memory and task empty-state CSS", () => {
    expect(memoryBrowserSource).not.toContain(".memory-empty {");
    expect(memoryBrowserSource).not.toContain(".empty-state {");
    expect(traceTimelineSource).not.toContain(".empty-hint {");
    expect(taskStepsSource).not.toContain(".empty-hint {");
    expect(permissionCenterSource).not.toContain(".empty-state {");
    expect(skillDiscoverListSource).not.toContain(".catalog-state {");
    expect(skillDiscoverListSource).not.toContain(".spinner {");
  });
});
