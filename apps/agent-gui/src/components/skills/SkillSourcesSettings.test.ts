import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { createPinia, setActivePinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import { commands, type SkillSourceView } from "@/generated/commands";
import skillSourcesSettingsSource from "./SkillSourcesSettings.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import SkillSourcesSettings from "./SkillSourcesSettings.vue";

vi.mock("@/generated/commands", () => ({
  commands: {
    listSkillSources: vi.fn(),
    addSkillSource: vi.fn(),
    removeSkillSource: vi.fn(),
    setSkillSourceEnabled: vi.fn()
  }
}));

const mockedCommands = vi.mocked(commands);

function createSource(overrides: Partial<SkillSourceView> = {}): SkillSourceView {
  return {
    id: "skillhub",
    display_name: "SkillHub",
    kind: "skillhub",
    url: "https://skills.palebluedot.live",
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

function mountSources() {
  return mountWithPlugins(SkillSourcesSettings, { reusePinia: true }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockedCommands.listSkillSources.mockResolvedValue([
    createSource(),
    createSource({
      id: "team-skillhub",
      display_name: "Team SkillHub",
      url: "https://skills.internal.example",
      search_template: "/internal/skills?keyword={{query}}",
      enabled: false,
      last_error: "Timeout contacting mirror"
    })
  ]);
  mockedCommands.addSkillSource.mockResolvedValue(null);
  mockedCommands.removeSkillSource.mockResolvedValue(null);
  mockedCommands.setSkillSourceEnabled.mockResolvedValue(null);
});

describe("SkillSourcesSettings", () => {
  it("uses shared form controls and action rows in the add-source form", () => {
    expectSourceMigration(skillSourcesSettingsSource, {
      required: [
        "SettingsFilterBar",
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

  it("filters configured skill sources by searchable source fields", async () => {
    const wrapper = mountSources();
    await flushPromises();

    expect(mockedCommands.listSkillSources).toHaveBeenCalledOnce();
    expect(wrapper.find('[data-test="skill-source-search-input"]').exists()).toBe(true);

    await wrapper.find('[data-test="skill-source-search-input"]').setValue("internal");

    expect(wrapper.find('[data-test="skill-source-row-team-skillhub"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="skill-source-row-skillhub"]').exists()).toBe(false);

    await wrapper.find('[data-test="skill-source-search-input"]').setValue("disabled");

    expect(wrapper.find('[data-test="skill-source-row-team-skillhub"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="skill-source-row-skillhub"]').exists()).toBe(false);

    await wrapper.find('[data-test="skill-source-search-input"]').setValue("timeout");

    expect(wrapper.find('[data-test="skill-source-row-team-skillhub"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="skill-source-row-skillhub"]').exists()).toBe(false);
  });

  it("shows a filtered empty state when no skill sources match search", async () => {
    const wrapper = mountSources();
    await flushPromises();

    await wrapper.find('[data-test="skill-source-search-input"]').setValue("does-not-exist");

    expect(wrapper.find('[data-test="skill-sources-list"]').exists()).toBe(false);
    const empty = wrapper.find('[data-test="skill-sources-filter-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("No skill catalog sources match your search.");
  });
});
