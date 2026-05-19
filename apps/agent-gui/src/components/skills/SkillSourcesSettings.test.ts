import { describe, expect, it } from "vitest";
import skillSourcesSettingsSource from "./SkillSourcesSettings.vue?raw";

describe("SkillSourcesSettings", () => {
  it("uses shared form controls and action rows in the add-source form", () => {
    expect(skillSourcesSettingsSource).toContain("SettingsCardList");
    expect(skillSourcesSettingsSource).toContain("SettingsCardItem");
    expect(skillSourcesSettingsSource).toContain("<template #actions>");
    expect(skillSourcesSettingsSource).toContain("KxFormActions");
    expect(skillSourcesSettingsSource).toContain("KxInput");
    expect(skillSourcesSettingsSource).toContain("KxSelect");
    expect(skillSourcesSettingsSource).not.toContain('class="src-actions"');
    expect(skillSourcesSettingsSource).not.toContain(".src-actions {");
    expect(skillSourcesSettingsSource).not.toContain("kx-form-control");
    expect(skillSourcesSettingsSource).not.toContain('class="input"');
    expect(skillSourcesSettingsSource).not.toContain(".input {");
    expect(skillSourcesSettingsSource).not.toContain(".form-actions {");
  });
});
