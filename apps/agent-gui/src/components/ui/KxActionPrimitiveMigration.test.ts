import { describe, expect, it } from "vitest";

import agentSettingsSource from "../AgentSettingsPane.vue?raw";
import catalogListSource from "../marketplace/CatalogList.vue?raw";
import mcpSettingsSource from "../McpSettingsPane.vue?raw";
import modelSettingsSource from "../ModelSettingsPane.vue?raw";
import pluginSettingsSource from "../PluginSettingsPane.vue?raw";
import skillSettingsSource from "../SkillSettingsPane.vue?raw";
import skillDiscoverListSource from "../skills/SkillDiscoverList.vue?raw";

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
    expect(agentSettingsSource).toContain("KxToolbarAction");
    expect(agentSettingsSource).toContain("KxInlineAction");
    expect(pluginSettingsSource).toContain("KxToolbarAction");
    expect(pluginSettingsSource).toContain("KxInlineAction");
    expect(skillSettingsSource).toContain("KxToolbarAction");
    expect(skillSettingsSource).toContain("KxInlineAction");
    expect(skillDiscoverListSource).toContain("KxToolbarAction");
    expect(skillDiscoverListSource).toContain("KxInlineAction");
  });
});
