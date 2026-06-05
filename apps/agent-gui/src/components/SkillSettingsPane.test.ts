import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { ref } from "vue";
import { mountWithPlugins } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import { commands, type SkillCatalogEntry, type EffectiveSkillView } from "@/generated/commands";
import SkillSettingsPane from "./SkillSettingsPane.vue";
import skillSettingsPaneSource from "./SkillSettingsPane.vue?raw";
import SkillSourcesSettings from "./skills/SkillSourcesSettings.vue";

vi.mock("@/generated/commands", () => ({
  commands: {
    listSkillSettings: vi.fn(),
    getEffectiveSkills: vi.fn(),
    setSkillEnabled: vi.fn(),
    deleteSkillSettings: vi.fn(),
    searchRemoteSkills: vi.fn(),
    listSkillCatalog: vi.fn(),
    listSkillSources: vi.fn(),
    addSkillSource: vi.fn(),
    removeSkillSource: vi.fn(),
    setSkillSourceEnabled: vi.fn(),
    refreshSkillCatalog: vi.fn(),
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
  tools: ["fs.read", "search.ripgrep"],
  can_request_tools: ["shell"],
  permission_summary: "tools: fs.read, search.ripgrep; can request: shell",
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
  tools: [],
  can_request_tools: [],
  permission_summary: "no tool permissions declared",
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
  tools: [],
  can_request_tools: [],
  permission_summary: "no tool permissions declared",
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
  tools: [],
  can_request_tools: [],
  permission_summary: "no tool permissions declared",
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
  package: "docs-helper",
  package_url: "https://api.skillhub.cn/api/v1/download?slug=docs-helper"
};

function mountPane(configSource?: "user" | "project", locale?: "en" | "zh-CN") {
  return mountWithPlugins(SkillSettingsPane, {
    reusePinia: true,
    locale,
    mount: configSource
      ? {
          global: {
            provide: {
              configSource: ref(configSource),
              configProjectId: ref("test-project")
            }
          }
        }
      : undefined
  }).wrapper;
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
  mockedCommands.addSkillSource.mockResolvedValue(null);
  mockedCommands.removeSkillSource.mockResolvedValue(null);
  mockedCommands.setSkillSourceEnabled.mockResolvedValue(null);
  mockedCommands.refreshSkillCatalog.mockResolvedValue(null);
  mockedCommands.installRemoteSkill.mockResolvedValue(projectSkill);
  mockedCommands.installGithubSkill.mockResolvedValue(projectSkill);
  mockedCommands.updateSkill.mockResolvedValue(projectSkill);
});

describe("SkillSettingsPane", () => {
  it("does not keep skill pane aria or install placeholder copy inline in the component source", () => {
    expectSourceMigration(skillSettingsPaneSource, {
      forbidden: [
        'aria-label="Skills settings"',
        'aria-label="Skill sections"',
        'placeholder="https://github.com/org/repo/tree/main/path/to/skill"',
        'aria-label="Search installed skills"',
        'placeholder="Search installed skills"',
        "Original order",
        "No installed skills match your search.",
        "SettingsEffectiveAudit",
        "skill-audit-"
      ]
    });
  });

  it("localizes installed skill search and sort controls in Chinese", async () => {
    const wrapper = mountPane(undefined, "zh-CN");
    await flushPromises();

    expect(
      wrapper.get('[data-test="skill-installed-search-input"]').attributes("placeholder")
    ).toBe("搜索已安装技能");
    expect(wrapper.get('[data-test="skill-installed-sort-select"]').text()).toContain("原始顺序");
  });

  it("renders installed skills with scope, enabled, activation, effective, update, and invalid states", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(mockedCommands.listSkillSettings).toHaveBeenCalledTimes(1);
    expect(wrapper.find('[data-test="skill-installed-search-input"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="skill-installed-list"]').classes()).toContain(
      "settings-card-list"
    );
    expect(wrapper.find('[data-test="skill-row-project-code-review"]').classes()).toContain(
      "settings-card-item"
    );
    expect(wrapper.find('[data-test="skill-row-project-code-review"]').text()).toContain("Project");
    expect(wrapper.find('[data-test="skill-row-project-code-review"]').text()).toContain("manual");
    expect(wrapper.find('[data-test="skill-row-project-code-review"]').text()).toContain(
      "update available"
    );
    expect(wrapper.find('[data-test="skill-permissions-project-code-review"]').text()).toContain(
      "fs.read, search.ripgrep"
    );
    expect(wrapper.find('[data-test="skill-permissions-project-code-review"]').text()).toContain(
      "shell"
    );
    expect(wrapper.find('[data-test="skill-row-user-test-driven-development"]').text()).toContain(
      "shadowed by project"
    );
    const shadowedAudit = wrapper.find('[data-test="skill-audit-user-test-driven-development"]');
    expect(shadowedAudit.exists()).toBe(false);
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

  it("filters installed skills by search text", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="skill-installed-search-input"]').setValue("review");

    expect(wrapper.find('[data-test="skill-row-project-code-review"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="skill-row-user-test-driven-development"]').exists()).toBe(
      false
    );
    expect(wrapper.find('[data-test="skill-row-builtin-builtin-planning"]').exists()).toBe(false);
  });

  it("sorts installed skills after applying search", async () => {
    const zetaTeamSkill = {
      ...projectSkill,
      settings_id: "project:zeta-team",
      id: "zeta-team",
      name: "Zeta Team",
      description: "Team workflow.",
      path: "/repo/.kairox/skills/zeta-team",
      update_state: "unknown"
    };
    const betaDocsSkill = {
      ...projectSkill,
      settings_id: "project:beta-docs",
      id: "beta-docs",
      name: "Beta Docs",
      description: "Documentation workflow.",
      path: "/repo/.kairox/skills/beta-docs"
    };
    const alphaTeamSkill = {
      ...projectSkill,
      settings_id: "user:alpha-team",
      id: "alpha-team",
      name: "Alpha Team",
      description: "Team workflow.",
      scope: "user",
      path: "/home/user/.kairox/skills/alpha-team",
      update_state: "up_to_date"
    };
    const settingsFixtures = [zetaTeamSkill, betaDocsSkill, alphaTeamSkill];
    const effectiveFixtures = settingsFixtures.map(toEffective);
    mockedCommands.listSkillSettings.mockResolvedValue(settingsFixtures);
    mockedCommands.getEffectiveSkills.mockResolvedValue(effectiveFixtures);

    const wrapper = mountPane();
    await flushPromises();
    const rowIds = () =>
      wrapper.findAll('[data-test^="skill-row-"]').map((row) => row.attributes("data-test"));

    const sortSelect = wrapper.get('[data-test="skill-installed-sort-select"]');
    expect(sortSelect.attributes("aria-label")).toBe("Installed skill sort");

    await wrapper.find('[data-test="skill-installed-search-input"]').setValue("team");
    expect(rowIds()).toEqual(["skill-row-project-zeta-team", "skill-row-user-alpha-team"]);

    await sortSelect.setValue("name");
    expect(rowIds()).toEqual(["skill-row-user-alpha-team", "skill-row-project-zeta-team"]);
    expect(effectiveFixtures.map((skill) => skill.value.settings_id)).toEqual([
      "project:zeta-team",
      "project:beta-docs",
      "user:alpha-team"
    ]);
  });

  it("matches installed skill search against metadata", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="skill-installed-search-input"]').setValue("check failed");

    expect(wrapper.find('[data-test="skill-row-project-broken-skill"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="skill-row-project-code-review"]').exists()).toBe(false);
  });

  it("shows a filtered empty state when no installed skills match search", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="skill-installed-search-input"]').setValue("does-not-exist");

    expect(wrapper.find('[data-test="skill-installed-list"]').exists()).toBe(false);
    const empty = wrapper.find('[data-test="skill-installed-filter-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("No installed skills match your search.");
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
      limit: 100
    });
    expect(wrapper.find('[data-test="skill-catalog-card"]').text()).toContain("42 installs");

    // Install button uses catalog_id-based data-test
    await wrapper.find('[data-test="skill-catalog-install-docs-helper"]').trigger("click");
    await flushPromises();

    // Default install target is "user" (syncs with ConfigSourceBar default)
    expect(mockedCommands.installRemoteSkill).toHaveBeenCalledWith({
      package: "docs-helper",
      package_url: "https://api.skillhub.cn/api/v1/download?slug=docs-helper",
      source: "registry",
      target: "user"
    });
    expect(mockedCommands.getEffectiveSkills).toHaveBeenCalledTimes(2);
  });

  it("opens a skill catalog detail drawer and installs into the selected project target", async () => {
    const wrapper = mountPane("project");
    await flushPromises();

    await wrapper.find('[data-test="skill-subtab-discover"]').trigger("click");
    await flushPromises();

    await wrapper.find('[data-test="skill-catalog-card"] button').trigger("click");
    await flushPromises();

    expect(document.body.textContent).toContain("Install target");
    expect(document.body.textContent).toContain("Project");

    document
      .querySelector<HTMLButtonElement>('[data-test="skill-catalog-detail-install"]')
      ?.click();
    await flushPromises();

    expect(mockedCommands.installRemoteSkill).toHaveBeenCalledWith({
      package: "docs-helper",
      package_url: "https://api.skillhub.cn/api/v1/download?slug=docs-helper",
      source: "registry",
      target: "project"
    });
    expect(mockedCommands.getEffectiveSkills).toHaveBeenCalledTimes(2);
  });

  it("installs skills from GitHub from the marketplace advanced install section", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="skill-github-form"]').exists()).toBe(false);

    await wrapper.find('[data-test="skill-subtab-discover"]').trigger("click");
    await flushPromises();

    await wrapper
      .find('[data-test="skill-github-source"]')
      .setValue("https://github.com/acme/skills/tree/main/packages/review");
    await wrapper.find('[data-test="skill-github-form"]').trigger("submit");
    await flushPromises();

    // Default install target is "user" (syncs with ConfigSourceBar default)
    expect(mockedCommands.installGithubSkill).toHaveBeenCalledWith({
      source: "https://github.com/acme/skills/tree/main/packages/review",
      target: "user"
    });
    expect(mockedCommands.getEffectiveSkills).toHaveBeenCalledTimes(2);
  });

  it("does not keep local skill row chrome after moving to SettingsCardItem", () => {
    expectSourceMigration(skillSettingsPaneSource, {
      required: [
        "SettingsCardList",
        "SettingsCardItem",
        "SettingsItemSummary",
        "SettingsItemMeta",
        "SettingsStatusTag"
      ],
      forbidden: [
        ".skill-settings__row,",
        ".skill-settings__row {",
        ".skill-settings__title-row",
        ".skill-settings__meta",
        "tag-success",
        "tag-warning",
        "tag-error",
        "tag--source",
        "tag--override",
        "tag--disabled-by"
      ]
    });
  });

  it("uses a compact installed skill card layout without path-driven blank space", async () => {
    const wrapper = mountPane();
    await flushPromises();

    const list = wrapper.find('[data-test="skill-installed-list"]');
    const row = wrapper.find('[data-test="skill-row-project-code-review"]');

    expect(list.classes()).toContain("settings-card-list--dense");
    expect(row.classes()).toContain("settings-card-item--compact");
    expectSourceMigration(skillSettingsPaneSource, {
      required: ['layout="stack"', ':description-lines="2"', 'density="compact"']
    });
  });

  it("keeps the installed skill filters from stretching inside the scroll body", () => {
    expectSourceMigration(skillSettingsPaneSource, {
      requiredPatterns: [/\.skill-settings__body[\s\S]*align-content:\s*start/]
    });
  });

  it("uses shared settings toolbar and subtabs instead of local skill chrome", () => {
    expectSourceMigration(skillSettingsPaneSource, {
      required: ["SettingsSubtabs", "SettingsToolbar", "SettingsFilterBar"],
      forbidden: [
        'class="skill-sub-tabs"',
        'class="skill-toolbar"',
        ".skill-sub-tabs {",
        ".skill-toolbar {",
        ".sub-tab-btn {"
      ]
    });
  });

  it("uses shared form controls in the GitHub advanced install form", () => {
    expectSourceMigration(skillSettingsPaneSource, {
      required: ["KxFormField", "KxInput"],
      forbidden: [
        "kx-form-control",
        ".skill-settings input,",
        ".skill-settings select",
        ".skill-settings__search-form input"
      ]
    });
  });

  it("uses shared settings state chrome for empty installed skills", async () => {
    mockedCommands.listSkillSettings.mockResolvedValue([]);
    mockedCommands.getEffectiveSkills.mockResolvedValue([]);

    const wrapper = mountPane();
    await flushPromises();

    const empty = wrapper.find('[data-test="skill-empty-state"]');
    expect(empty.exists()).toBe(true);
    expect(empty.classes()).toContain("settings-state");
    expect(empty.text()).toContain("No skills installed yet.");
  });

  it("adds a skill source with required search and download templates", async () => {
    const wrapper = mountWithPlugins(SkillSourcesSettings, { reusePinia: true }).wrapper;
    await flushPromises();

    await wrapper.find('[data-test="skill-add-source-toggle"]').trigger("click");
    await wrapper.find('[data-test="skill-src-id"]').setValue("custom-skillhub");
    await wrapper.find('[data-test="skill-src-name"]').setValue("Custom SkillHub");
    await wrapper.find('[data-test="skill-src-url"]').setValue("https://api.skillhub.cn");
    await wrapper.find('[data-test="skill-src-list-template"]').setValue("");
    await wrapper.find('[data-test="skill-src-detail-template"]').setValue("");
    await wrapper.find('[data-test="skill-src-save"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.addSkillSource).toHaveBeenCalledWith(
      expect.objectContaining({
        id: "custom-skillhub",
        display_name: "Custom SkillHub",
        search_template:
          "/api/skills?keyword={{query}}&page=1&pageSize={{limit}}&sortBy=downloads&order=desc",
        download_template: "/api/v1/download?slug={{slug}}",
        list_template: null,
        detail_template: null
      })
    );
  });
});
