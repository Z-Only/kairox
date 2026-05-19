import { describe, expect, it } from "vitest";
import agentSettingsPaneSource from "../AgentSettingsPane.vue?raw";
import archiveSettingsPaneSource from "../ArchiveSettingsPane.vue?raw";
import hooksSettingsPaneSource from "../HooksSettingsPane.vue?raw";
import mcpServerCardSource from "../McpServerCard.vue?raw";
import modelProfileCardSource from "../ModelProfileCard.vue?raw";
import pluginSettingsPaneSource from "../PluginSettingsPane.vue?raw";
import skillSettingsPaneSource from "../SkillSettingsPane.vue?raw";

const migratedSources = [
  ["AgentSettingsPane.vue", agentSettingsPaneSource, "agent-row__actions"],
  ["ArchiveSettingsPane.vue", archiveSettingsPaneSource, "archive-row__actions"],
  ["HooksSettingsPane.vue", hooksSettingsPaneSource, "hook-row__actions"],
  ["McpServerCard.vue", mcpServerCardSource, "mcp-settings__actions"],
  ["ModelProfileCard.vue", modelProfileCardSource, "model-settings__actions"],
  ["PluginSettingsPane.vue", pluginSettingsPaneSource, "plugin-actions"],
  ["SkillSettingsPane.vue", skillSettingsPaneSource, "skill-settings__actions"]
] as const;

describe("settings action group migration", () => {
  it.each(migratedSources)(
    "%s uses SettingsCardItem actions slot instead of local action wrappers",
    (_filename, source, legacyClass) => {
      expect(source).toContain("<template #actions>");
      expect(source).not.toContain(`class="${legacyClass}"`);
      expect(source).not.toContain(`.${legacyClass} {`);
    }
  );
});
