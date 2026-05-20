import { describe, expect, it } from "vitest";

import agentSettingsSource from "../AgentSettingsPane.vue?raw";
import catalogListSource from "../marketplace/CatalogList.vue?raw";
import mcpSettingsSource from "../McpSettingsPane.vue?raw";
import modelSettingsSource from "../ModelSettingsPane.vue?raw";
import pluginSettingsSource from "../PluginSettingsPane.vue?raw";
import skillSettingsSource from "../SkillSettingsPane.vue?raw";
import skillDiscoverListSource from "../skills/SkillDiscoverList.vue?raw";
import { expectSourceToContain } from "@/test-utils/sourceGuards";

const migratedSources = [
  agentSettingsSource,
  catalogListSource,
  mcpSettingsSource,
  modelSettingsSource,
  pluginSettingsSource,
  skillSettingsSource,
  skillDiscoverListSource
];

describe("settings action primitive migration", () => {
  it("keeps settings toolbar and inline actions off ad-hoc small KxButton declarations", () => {
    for (const source of migratedSources) {
      expect(source).not.toMatch(/<KxButton[^>]*\bsize="sm"/);
    }
  });

  it("uses semantic toolbar and inline action wrappers in migrated settings panes", () => {
    for (const source of [
      agentSettingsSource,
      pluginSettingsSource,
      skillSettingsSource,
      skillDiscoverListSource
    ]) {
      expectSourceToContain(source, ["KxToolbarAction", "KxInlineAction"]);
    }
  });
});
