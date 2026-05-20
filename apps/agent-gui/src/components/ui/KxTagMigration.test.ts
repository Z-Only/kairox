import { describe, it } from "vitest";
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
import { expectSourceMigration } from "@/test-utils/sourceGuards";

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
    expectSourceMigration(source, {
      forbidden: legacyClassFragments,
      requiredPatterns: [/Kx(Tag|Badge)/],
      forbiddenPatterns: [/class="[^"]*\btag\b/, /:class="[^"]*\btag\b/]
    });
  });

  it("keeps only the neutral compatibility tag base in shared CSS", () => {
    expectSourceMigration(componentsCssSource, {
      required: [".tag {"],
      forbidden: legacyClassFragments.map((fragment) => `.${fragment}`)
    });
  });
});
