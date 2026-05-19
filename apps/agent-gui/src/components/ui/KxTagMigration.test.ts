import { describe, expect, it } from "vitest";
import { readFileSync } from "node:fs";

import catalogCardSource from "@/components/marketplace/CatalogCard.vue?raw";
import installedListSource from "@/components/marketplace/InstalledList.vue?raw";
import skillDiscoverCardSource from "@/components/skills/SkillDiscoverCard.vue?raw";
import memoryBrowserSource from "@/components/MemoryBrowser.vue?raw";
import chatPanelSource from "@/components/ChatPanel.vue?raw";
import mcpServerManagerSource from "@/components/McpServerManager.vue?raw";
import traceEntrySource from "@/components/TraceEntry.vue?raw";
import taskNodeSource from "@/components/TaskNode.vue?raw";
import permissionPromptSource from "@/components/PermissionPrompt.vue?raw";
import mcpResourceAccordionSource from "@/components/McpResourceAccordion.vue?raw";
import mcpPromptAccordionSource from "@/components/McpPromptAccordion.vue?raw";
import marketplacePaneSource from "@/components/MarketplacePane.vue?raw";

const componentsCssSource = readFileSync("src/styles/components.css", "utf8");

const migratedSources = [
  ["CatalogCard.vue", catalogCardSource],
  ["InstalledList.vue", installedListSource],
  ["SkillDiscoverCard.vue", skillDiscoverCardSource],
  ["MemoryBrowser.vue", memoryBrowserSource],
  ["ChatPanel.vue", chatPanelSource],
  ["McpServerManager.vue", mcpServerManagerSource],
  ["TraceEntry.vue", traceEntrySource],
  ["TaskNode.vue", taskNodeSource],
  ["PermissionPrompt.vue", permissionPromptSource],
  ["McpResourceAccordion.vue", mcpResourceAccordionSource],
  ["McpPromptAccordion.vue", mcpPromptAccordionSource],
  ["MarketplacePane.vue", marketplacePaneSource]
] as const;

const legacyClassFragments = [
  "tag-success",
  "tag-warning",
  "tag-error",
  "tag-info",
  "tag-default",
  "tag-sm",
  "tag--mime",
  "tag-source",
  "tag-link"
];

describe("KxTag migration", () => {
  it.each(migratedSources)("%s uses KxTag/KxBadge instead of direct tag markup", (_, source) => {
    expect(source).toMatch(/Kx(Tag|Badge)/);
    expect(source).not.toMatch(/class="[^"]*\btag\b/);
    expect(source).not.toMatch(/:class="[^"]*\btag\b/);
    for (const fragment of legacyClassFragments) {
      expect(source).not.toContain(fragment);
    }
  });

  it("keeps only the neutral compatibility tag base in shared CSS", () => {
    expect(componentsCssSource).toContain(".tag {");
    for (const fragment of legacyClassFragments) {
      expect(componentsCssSource).not.toContain(`.${fragment}`);
    }
  });
});
