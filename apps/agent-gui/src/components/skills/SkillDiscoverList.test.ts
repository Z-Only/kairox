import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import { commands, type SkillCatalogEntry, type SkillSourceView } from "@/generated/commands";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import SkillDiscoverList from "./SkillDiscoverList.vue";
import skillDiscoverListSource from "./SkillDiscoverList.vue?raw";

vi.mock("@/generated/commands", () => ({
  commands: {
    listSkillCatalog: vi.fn(),
    listSkillSources: vi.fn(),
    refreshSkillCatalog: vi.fn(),
    installRemoteSkill: vi.fn(),
    getEffectiveSkills: vi.fn()
  }
}));

const mockedCommands = vi.mocked(commands);

const docsHelper: SkillCatalogEntry = {
  catalog_id: "skillhub/docs-helper",
  name: "Docs Helper",
  description: "Summarize documentation.",
  source: "skillhub",
  source_url: "https://registry.example/docs-helper",
  install_count: 42,
  github_stars: 12,
  security_score: 95,
  rating: 4.9,
  package: "skillhub/docs-helper",
  package_url: "https://registry.example/download/docs-helper"
};

function createCatalogEntry(overrides: Partial<SkillCatalogEntry>): SkillCatalogEntry {
  return {
    ...docsHelper,
    ...overrides
  };
}

function createSource(overrides: Partial<SkillSourceView> = {}): SkillSourceView {
  return {
    id: "skillhub",
    display_name: "SkillHub",
    kind: "skillhub",
    url: "https://skills.example",
    search_template: "/api/skills?q={{query}}&limit={{limit}}",
    download_template: "/api/download/{{slug}}",
    list_template: "/api/skills?limit={{limit}}",
    detail_template: null,
    field_mapping: {
      name_path: "name",
      description_path: "description",
      install_count_path: "downloads",
      github_stars_path: "stars",
      package_path: "id",
      source_url_path: "sourceUrl"
    },
    enabled: true,
    priority: 10,
    cache_ttl_seconds: 900,
    last_error: null,
    ...overrides
  };
}

function mountList() {
  return mountWithPlugins(SkillDiscoverList, {
    reusePinia: true,
    mount: {
      props: {
        installTarget: "user"
      }
    }
  }).wrapper;
}

function catalogCardNames(wrapper: ReturnType<typeof mountList>): string[] {
  return wrapper.findAll('[data-test="skill-catalog-card"]').map((card) => {
    return card.get(".display-name").text();
  });
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockedCommands.listSkillSources.mockResolvedValue([createSource()]);
  mockedCommands.listSkillCatalog.mockResolvedValue([docsHelper]);
  mockedCommands.refreshSkillCatalog.mockResolvedValue(null);
  mockedCommands.installRemoteSkill.mockResolvedValue({
    settings_id: "user:docs-helper",
    id: "docs-helper",
    name: "Docs Helper",
    description: "Summarize documentation.",
    version: "1.0.0",
    scope: "user",
    path: "/Users/mock/.kairox/skills/docs-helper/SKILL.md",
    enabled: true,
    activation_mode: "manual",
    install_source: "registry",
    update_state: "up_to_date",
    effective: true,
    shadowed_by: null,
    valid: true,
    validation_error: null,
    editable: true,
    deletable: true
  });
  mockedCommands.getEffectiveSkills.mockResolvedValue([]);
});

describe("SkillDiscoverList", () => {
  it("loads the marketplace with search controls and a larger discovery limit", async () => {
    const wrapper = mountList();
    await flushPromises();

    expect(mockedCommands.listSkillSources).toHaveBeenCalledOnce();
    expect(mockedCommands.listSkillCatalog).toHaveBeenCalledWith({
      keyword: null,
      sources: null,
      limit: 100
    });
    expect(wrapper.get('[data-test="skill-catalog-search"]').exists()).toBe(true);
    expect(wrapper.get('[data-test="skill-catalog-refresh"]').exists()).toBe(true);
    expect(wrapper.get('[data-test="skill-source-filter-skillhub"]').text()).toBe("SkillHub");
  });

  it("searches with keyword and source filters without toggling source enablement", async () => {
    const wrapper = mountList();
    await flushPromises();

    await wrapper.get('[data-test="skill-source-filter-skillhub"]').trigger("click");
    await wrapper.get('[data-test="skill-catalog-search"]').setValue("review");
    await wrapper.get('[data-test="skill-catalog-search-btn"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.listSkillCatalog).toHaveBeenLastCalledWith({
      keyword: "review",
      sources: ["skillhub"],
      limit: 100
    });
    expect("setSkillSourceEnabled" in mockedCommands).toBe(false);
  });

  it("refreshes the catalog and reruns the active query", async () => {
    const wrapper = mountList();
    await flushPromises();

    await wrapper.get('[data-test="skill-catalog-search"]').setValue("docs");
    await wrapper.get('[data-test="skill-catalog-search-btn"]').trigger("click");
    await flushPromises();
    await wrapper.get('[data-test="skill-catalog-refresh"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.refreshSkillCatalog).toHaveBeenCalledOnce();
    expect(mockedCommands.listSkillCatalog).toHaveBeenLastCalledWith({
      keyword: "docs",
      sources: null,
      limit: 100
    });
  });

  it("sorts the displayed catalog locally without running another backend search", async () => {
    mockedCommands.listSkillCatalog.mockResolvedValueOnce([
      createCatalogEntry({
        catalog_id: "skillhub/beta",
        name: "Beta",
        install_count: 12,
        github_stars: 40,
        security_score: 72,
        rating: 4.2,
        package: "skillhub/beta"
      }),
      createCatalogEntry({
        catalog_id: "skillhub/gamma",
        name: "Gamma",
        install_count: 30,
        github_stars: 5,
        security_score: 91,
        rating: 4.7,
        package: "skillhub/gamma"
      }),
      createCatalogEntry({
        catalog_id: "skillhub/alpha",
        name: "Alpha",
        install_count: 30,
        github_stars: 80,
        security_score: 88,
        rating: 4.7,
        package: "skillhub/alpha"
      })
    ]);
    const wrapper = mountList();
    await flushPromises();

    expect(catalogCardNames(wrapper)).toEqual(["Beta", "Gamma", "Alpha"]);
    expect(mockedCommands.listSkillCatalog).toHaveBeenCalledTimes(1);

    const sortSelect = wrapper.get('[data-test="skill-catalog-sort-select"]');
    expect(sortSelect.attributes("aria-label")).toBe("Skill catalog sort");

    await sortSelect.setValue("name");
    await flushPromises();

    expect(catalogCardNames(wrapper)).toEqual(["Alpha", "Beta", "Gamma"]);
    expect(mockedCommands.listSkillCatalog).toHaveBeenCalledTimes(1);

    await sortSelect.setValue("downloads");
    await flushPromises();

    expect(catalogCardNames(wrapper)).toEqual(["Gamma", "Alpha", "Beta"]);
    expect(mockedCommands.listSkillCatalog).toHaveBeenCalledTimes(1);
  });

  it("renders empty and error states with recovery actions", async () => {
    mockedCommands.listSkillCatalog.mockResolvedValueOnce([]);
    const wrapper = mountList();
    await flushPromises();

    expect(wrapper.get('[data-test="skill-catalog-empty"]').text()).toContain(
      "No skills match the current filters."
    );

    mockedCommands.listSkillCatalog.mockRejectedValueOnce(new Error("catalog unavailable"));
    await wrapper.get('[data-test="skill-catalog-retry"]').trigger("click");
    await flushPromises();

    expect(wrapper.get('[data-test="skill-catalog-error"]').text()).toContain(
      "catalog unavailable"
    );
  });

  it("shows install completion feedback on the installed card", async () => {
    const wrapper = mountList();
    await flushPromises();

    await wrapper.get('[data-test="skill-catalog-install-skillhub/docs-helper"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.installRemoteSkill).toHaveBeenCalledWith({
      package: "skillhub/docs-helper",
      package_url: "https://registry.example/download/docs-helper",
      source: "registry",
      target: "user"
    });
    expect(wrapper.get('[data-test="skill-catalog-install-success"]').text()).toContain(
      "Installed Docs Helper"
    );
    expect(wrapper.get('[data-test="skill-catalog-install-skillhub/docs-helper"]').text()).toBe(
      "Installed"
    );
  });

  it("uses shared filter bar instead of local discover toolbar chrome", () => {
    expectSourceMigration(skillDiscoverListSource, {
      required: ["SettingsFilterBar", "KxChipGroup", "KxChipButton", "KxToolbarAction"],
      forbidden: [
        'class="discover-toolbar"',
        ".discover-toolbar {",
        ".discover-search-row {",
        ".discover-search-input {",
        ".source-filter .chip"
      ]
    });
  });
});
