import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import { commands, type SkillCatalogEntry, type EffectiveSkillView } from "@/generated/commands";
import SkillSettingsPane from "./SkillSettingsPane.vue";

vi.mock("@/generated/commands", () => ({
  commands: {
    listSkillSettings: vi.fn(),
    getEffectiveSkills: vi.fn(),
    setSkillEnabled: vi.fn(),
    deleteSkillSettings: vi.fn(),
    searchRemoteSkills: vi.fn(),
    listSkillCatalog: vi.fn(),
    listSkillSources: vi.fn(),
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

function toEffective(skill: typeof projectSkill): EffectiveSkillView {
  return {
    value: skill,
    source: skill.scope === "project" ? "Project" : skill.scope === "builtin" ? "Builtin" : "User",
    overrides: skill.shadowed_by ? (skill.shadowed_by === "project" ? "Project" : "User") : null,
    enabled: skill.enabled,
    disabledBy: null,
    writable: skill.editable,
    deletable: skill.deletable
  };
}

const remoteSkill: SkillCatalogEntry = {
  catalog_id: "docs-helper",
  name: "Docs Helper",
  description: "Summarize documentation.",
  source: "registry",
  source_url: "https://registry.example/docs-helper",
  install_count: 42,
  github_stars: null,
  security_score: null,
  rating: null,
  package: "docs-helper"
};

function mountPane() {
  return mountWithPlugins(SkillSettingsPane, { reusePinia: true }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  const settingsFixtures = [projectSkill, shadowedSkill, builtinSkill, invalidSkill];
  mockedCommands.listSkillSettings.mockResolvedValue(settingsFixtures);
  mockedCommands.getEffectiveSkills.mockResolvedValue(settingsFixtures.map(toEffective));
  mockedCommands.setSkillEnabled.mockResolvedValue(null);
  mockedCommands.deleteSkillSettings.mockResolvedValue(null);
  mockedCommands.searchRemoteSkills.mockResolvedValue([remoteSkill]);
  mockedCommands.listSkillCatalog.mockResolvedValue([remoteSkill]);
  mockedCommands.listSkillSources.mockResolvedValue([]);
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

  it("does not render edit buttons for skills", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="skill-edit-project-code-review"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="skill-edit-builtin-builtin-planning"]').exists()).toBe(false);
  });

  it("toggles enabled state through the skills store action", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="skill-enabled-project-code-review"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.setSkillEnabled).toHaveBeenCalledWith("project:code-review", false);
  });

  it("uses unique settings selectors for shadowed duplicate skill ids", async () => {
    const userShadowedSkill = {
      ...projectSkill,
      settings_id: "user:code-review",
      scope: "user",
      path: "/home/user/.kairox/skills/code-review",
      effective: false,
      shadowed_by: "project"
    };
    const settingsFixtures = [projectSkill, userShadowedSkill];
    mockedCommands.listSkillSettings.mockResolvedValue(settingsFixtures);
    mockedCommands.getEffectiveSkills.mockResolvedValue(settingsFixtures.map(toEffective));

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

    // Search for skills using the catalog search input and button
    await wrapper.find('[data-test="skill-catalog-search"]').setValue("docs");
    await wrapper.find('[data-test="skill-catalog-search-btn"]').trigger("click");
    await flushPromises();

    // searchCatalog is called via the skills store
    expect(mockedCommands.listSkillCatalog).toHaveBeenCalledWith({
      keyword: "docs",
      sources: null,
      limit: 50
    });
    expect(wrapper.find('[data-test="skill-catalog-card"]').text()).toContain("42 installs");

    // Install button uses catalog_id-based data-test
    await wrapper.find('[data-test="skill-catalog-install-docs-helper"]').trigger("click");
    await flushPromises();

    // Default install target is "user" (syncs with ConfigSourceBar default)
    expect(mockedCommands.installRemoteSkill).toHaveBeenCalledWith({
      package: "docs-helper",
      package_url: null,
      source: "registry",
      target: "user"
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

    // Default install target is "user" (syncs with ConfigSourceBar default)
    expect(mockedCommands.installGithubSkill).toHaveBeenCalledWith({
      source: "https://github.com/acme/skill.git",
      target: "user"
    });
  });
});
