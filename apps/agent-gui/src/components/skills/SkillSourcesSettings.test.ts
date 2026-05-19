import { describe, expect, it } from "vitest";
import skillSourcesSettingsSource from "./SkillSourcesSettings.vue?raw";

describe("SkillSourcesSettings", () => {
  it("uses shared form controls and action rows in the add-source form", () => {
    expect(skillSourcesSettingsSource).toContain("KxFormActions");
    expect(skillSourcesSettingsSource).toContain("kx-form-control");
    expect(skillSourcesSettingsSource).not.toContain('class="input"');
    expect(skillSourcesSettingsSource).not.toContain(".input {");
    expect(skillSourcesSettingsSource).not.toContain(".form-actions {");
  });
});
