import { describe, it } from "vitest";
import skillSourcesSettingsSource from "./SkillSourcesSettings.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

describe("SkillSourcesSettings", () => {
  it("uses shared form controls and action rows in the add-source form", () => {
    expectSourceMigration(skillSourcesSettingsSource, {
      required: [
        "SettingsCardList",
        "SettingsCardItem",
        "SettingsStatusTag",
        "<template #actions>",
        "KxFormActions",
        "KxInput",
        "KxSelect"
      ],
      forbidden: [
        "tag-info",
        'class="src-actions"',
        ".src-actions {",
        "kx-form-control",
        'class="input"',
        ".input {",
        ".form-actions {"
      ]
    });
  });

  it("does not keep skill source aria, option, or form helper copy inline", () => {
    expectSourceMigration(skillSourcesSettingsSource, {
      forbidden: [
        'aria-label="Skill catalog sources"',
        'label="id"',
        'label: "SkillHub"',
        'placeholder="/api/v1/download?slug={{slug}}"',
        "Use {{query}} and {{limit}} tokens for search requests."
      ]
    });
  });
});
