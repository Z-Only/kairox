import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import { commands } from "@/generated/commands";
import SkillSettingsPane from "./SkillSettingsPane.vue";

vi.mock("@/generated/commands", () => ({
  commands: {
    listSkillSettings: vi.fn(),
    setSkillEnabled: vi.fn(),
    deleteSkillSettings: vi.fn(),
    searchRemoteSkills: vi.fn(),
    installRemoteSkill: vi.fn(),
    installGithubSkill: vi.fn(),
    updateSkill: vi.fn()
  }
}));

const mockedCommands = vi.mocked(commands);

const projectSkill = {
  settings_id: "project:code-review",
  id: "code-review",
  name: "Code Review",
  description: "Review implementation quality.",
  version: "1.0.0",
  scope: "project",
  path: "/repo/.kairox/skills/code-review",
  enabled: true,
  activation_mode: "manual",
  install_source: "github",
  update_state: "update_available",
  effective: true,
  shadowed_by: null,
  valid: true,
  validation_error: null,
  editable: true,
  deletable: true
};

const shadowedSkill = {
  settings_id: "user:test-driven-development",
  id: "test-driven-development",
  name: "TDD",
  description: "Test-first implementation.",
  version: null,
  scope: "user",
  path: "/home/user/.kairox/skills/tdd",
  enabled: true,
  activation_mode: "auto",
  install_source: "registry",
  update_state: "up_to_date",
  effective: false,
  shadowed_by: "project",
  valid: true,
  validation_error: null,
  editable: true,
  deletable: true
};

const builtinSkill = {
  settings_id: "builtin:builtin-planning",
  id: "builtin-planning",
  name: "Built-in Planning",
  description: "Plan work before editing.",
  version: "2.0.0",
  scope: "builtin",
  path: "builtin:/planning",
  enabled: true,
  activation_mode: "manual",
  install_source: "builtin",
  update_state: "unknown",
  effective: true,
  shadowed_by: null,
  valid: true,
  validation_error: null,
  editable: false,
  deletable: false
};

const invalidSkill = {
  settings_id: "project:broken-skill",
  id: "broken-skill",
  name: "Broken Skill",
  description: "Invalid fixture.",
  version: null,
  scope: "project",
  path: "/repo/.kairox/skills/broken",
  enabled: false,
  activation_mode: "manual",
  install_source: "local",
  update_state: "check_failed",
  effective: true,
  shadowed_by: null,
  valid: false,
  validation_error: "Missing SKILL.md frontmatter",
  editable: true,
  deletable: true
};

const remoteSkill = {
  name: "Docs Helper",
  description: "Summarize documentation.",
  repository: "https://github.com/acme/docs-helper",
  install_count: 42,
  source_url: "https://registry.example/docs-helper",
  package: "docs-helper"
};

function mountPane() {
  return mountWithPlugins(SkillSettingsPane, { reusePinia: true }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockedCommands.listSkillSettings.mockResolvedValue([
    projectSkill,
    shadowedSkill,
    builtinSkill,
    invalidSkill
  ]);
  mockedCommands.setSkillEnabled.mockResolvedValue(null);
  mockedCommands.deleteSkillSettings.mockResolvedValue(null);
  mockedCommands.searchRemoteSkills.mockResolvedValue([remoteSkill]);
  mockedCommands.installRemoteSkill.mockResolvedValue(projectSkill);
  mockedCommands.installGithubSkill.mockResolvedValue(projectSkill);
  mockedCommands.updateSkill.mockResolvedValue(projectSkill);
});

describe("SkillSettingsPane", () => {
  it("renders installed skills with scope, enabled, activation, effective, update, and invalid states", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(mockedCommands.listSkillSettings).toHaveBeenCalledTimes(1);
    expect(wrapper.find('[data-test="skill-row-project-code-review"]').text()).toContain("project");
    expect(wrapper.find('[data-test="skill-row-project-code-review"]').text()).toContain("manual");
    expect(wrapper.find('[data-test="skill-row-project-code-review"]').text()).toContain(
      "update available"
    );
    expect(wrapper.find('[data-test="skill-row-user-test-driven-development"]').text()).toContain(
      "shadowed by project"
    );
    expect(wrapper.find('[data-test="skill-row-project-broken-skill"]').text()).toContain(
      "Missing SKILL.md frontmatter"
    );
  });

  it("keeps skill edit actions read-only until an editor exists", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(
      wrapper.find<HTMLButtonElement>('[data-test="skill-edit-project-code-review"]').element
        .disabled
    ).toBe(true);
    expect(
      wrapper.find<HTMLButtonElement>('[data-test="skill-edit-builtin-builtin-planning"]').element
        .disabled
    ).toBe(true);
    expect(
      wrapper.find<HTMLButtonElement>('[data-test="skill-delete-builtin-builtin-planning"]').element
        .disabled
    ).toBe(true);
    expect(
      wrapper.find<HTMLButtonElement>('[data-test="skill-update-builtin-builtin-planning"]').element
        .disabled
    ).toBe(true);
  });

  it("toggles enabled state through the skills store action", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="skill-enabled-project-code-review"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.setSkillEnabled).toHaveBeenCalledWith("project:code-review", false);
  });

  it("uses unique settings selectors for shadowed duplicate skill ids", async () => {
    mockedCommands.listSkillSettings.mockResolvedValue([
      projectSkill,
      {
        ...projectSkill,
        settings_id: "user:code-review",
        scope: "user",
        path: "/home/user/.kairox/skills/code-review",
        effective: false,
        shadowed_by: "project"
      }
    ]);

    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.findAll('[data-test="skill-row-project-code-review"]')).toHaveLength(1);
    expect(wrapper.findAll('[data-test="skill-row-user-code-review"]')).toHaveLength(1);

    await wrapper.find('[data-test="skill-enabled-user-code-review"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.setSkillEnabled).toHaveBeenCalledWith("user:code-review", false);
  });

  it("discovers remote skills and installs a selected result", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="skill-subtab-discover"]').trigger("click");
    await flushPromises();

    await wrapper.find('[data-test="skill-discover-query"]').setValue("docs");
    await wrapper.find('[data-test="skill-discover-form"]').trigger("submit");
    await flushPromises();

    expect(mockedCommands.searchRemoteSkills).toHaveBeenCalledWith("docs");
    expect(wrapper.find('[data-test="skill-remote-docs-helper"]').text()).toContain("42 installs");

    await wrapper.find('[data-test="skill-install-docs-helper"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.installRemoteSkill).toHaveBeenCalledWith({
      package: "docs-helper",
      source: "docs-helper",
      target: "project"
    });
  });

  it("installs skills from GitHub with a stable form selector", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper
      .find('[data-test="skill-github-source"]')
      .setValue("https://github.com/acme/skill.git");
    await wrapper.find('[data-test="skill-github-form"]').trigger("submit");
    await flushPromises();

    expect(mockedCommands.installGithubSkill).toHaveBeenCalledWith({
      source: "https://github.com/acme/skill.git",
      target: "project"
    });
  });
});
