import { describe, it, expect, vi, beforeEach, beforeAll } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { ref } from "vue";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import { commands, type EffectiveProfileView } from "@/generated/commands";
import ModelSettingsPane from "./ModelSettingsPane.vue";
import modelSettingsPaneSource from "./ModelSettingsPane.vue?raw";
import modelProfileCardSource from "./ModelProfileCard.vue?raw";

beforeAll(() => {
  HTMLDialogElement.prototype.showModal ??= vi.fn();
  HTMLDialogElement.prototype.close ??= vi.fn();
});

vi.mock("@/generated/commands", () => ({
  commands: {
    listProfileSettings: vi.fn(),
    getEffectiveModelProfiles: vi.fn(),
    upsertProfileSettings: vi.fn(),
    setProfileEnabled: vi.fn(),
    deleteProfileSettings: vi.fn(),
    moveProfileInOrder: vi.fn(),
    openProfilesConfigFile: vi.fn(),
    testModelConnectivity: vi.fn()
  }
}));

const mockedCommands = vi.mocked(commands);

const writableProfile = {
  alias: "my-model",
  provider: "openai_compatible",
  model_id: "gpt-4.1",
  enabled: true,
  context_window: 128000,
  output_limit: 16384,
  temperature: 0.7,
  top_p: null,
  top_k: null,
  max_tokens: null,
  base_url: "https://api.openai.com/v1",
  api_key_env: "OPENAI_API_KEY",
  has_api_key: true,
  writable: true,
  config_path: "/tmp/profiles.toml",
  source: "profiles_toml"
};

const readOnlyProfile = {
  alias: "fast",
  provider: "openai_compatible",
  model_id: "gpt-4.1-mini",
  enabled: true,
  context_window: 128000,
  output_limit: 16384,
  temperature: null,
  top_p: null,
  top_k: null,
  max_tokens: null,
  base_url: "https://api.openai.com/v1",
  api_key_env: "OPENAI_API_KEY",
  has_api_key: true,
  writable: false,
  config_path: null,
  source: "user_config"
};

const projectOnlyProfile = {
  alias: "local-code",
  provider: "anthropic",
  model_id: "claude-opus-4-7",
  enabled: false,
  context_window: 200000,
  output_limit: 32000,
  temperature: null,
  top_p: null,
  top_k: null,
  max_tokens: null,
  base_url: null,
  api_key_env: null,
  has_api_key: false,
  writable: true,
  config_path: "/tmp/profiles.toml",
  source: "project_config"
};

function toEffective(
  profile: typeof writableProfile | typeof readOnlyProfile | typeof projectOnlyProfile
): EffectiveProfileView {
  return {
    value: profile,
    source: profile.source === "project_config" ? "Project" : "User",
    overrides: null,
    enabled: profile.enabled,
    disabledBy: null,
    writable: profile.writable,
    deletable: profile.writable
  };
}

function ok<T>(data: T): { status: "ok"; data: T } {
  return { status: "ok", data };
}

function mountPane(configSource?: "user" | "project") {
  const mountOptions: MountWithPluginsOptions<typeof ModelSettingsPane> = {
    reusePinia: true,
    mount: configSource
      ? {
          global: {
            provide: {
              configSource: ref(configSource),
              configProjectId: ref(undefined)
            }
          }
        }
      : undefined
  };
  return mountWithPlugins(ModelSettingsPane, mountOptions).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  const profileFixtures = [writableProfile, readOnlyProfile];
  mockedCommands.listProfileSettings.mockResolvedValue(ok(profileFixtures));
  mockedCommands.getEffectiveModelProfiles.mockResolvedValue(
    ok([...profileFixtures, projectOnlyProfile].map(toEffective))
  );
  mockedCommands.upsertProfileSettings.mockResolvedValue(ok(writableProfile));
  mockedCommands.setProfileEnabled.mockResolvedValue(ok(null));
  mockedCommands.deleteProfileSettings.mockResolvedValue(ok(null));
  mockedCommands.moveProfileInOrder.mockResolvedValue(ok(null));
  mockedCommands.openProfilesConfigFile.mockResolvedValue(ok("/tmp/profiles.toml"));
});

describe("ModelSettingsPane", () => {
  it("renders profile list with correct data", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    expect(wrapper.find('[data-test="model-row-my-model"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-search-input"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-list"]').classes()).toContain("settings-card-list");
    expect(wrapper.find('[data-test="model-row-my-model"]').classes()).toContain(
      "settings-card-item"
    );
    expect(wrapper.find('[data-test="model-row-fast"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-row-local-code"]').exists()).toBe(false);

    const myModelRow = wrapper.find('[data-test="model-row-my-model"]');
    expect(myModelRow.text()).toContain("Enabled");

    const fastRow = wrapper.find('[data-test="model-row-fast"]');
    expect(fastRow.text()).toContain("User config");
  });

  it("filters profiles by search text", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="model-search-input"]').setValue("fast");

    expect(wrapper.find('[data-test="model-row-fast"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-row-my-model"]').exists()).toBe(false);
  });

  it("matches search against model metadata", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="model-search-input"]').setValue("gpt-4.1-mini");

    expect(wrapper.find('[data-test="model-row-fast"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-row-my-model"]').exists()).toBe(false);
  });

  it("sorts filtered profiles without losing the search filter", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="model-search-input"]').setValue("gpt-4.1");

    expect(
      wrapper.findAll('[data-test^="model-row-"]').map((row) => row.attributes("data-test"))
    ).toEqual(["model-row-my-model", "model-row-fast"]);

    const sortSelect = wrapper.find('[data-test="model-sort-select"]');
    expect(sortSelect.exists()).toBe(true);
    expect(sortSelect.attributes("aria-label")).toBe("Model profile sort");
    expect(sortSelect.findAll("option").map((option) => option.attributes("value"))).toEqual([
      "original",
      "alias",
      "provider",
      "source",
      "status"
    ]);

    await sortSelect.setValue("provider");
    expect(
      wrapper.findAll('[data-test^="model-row-"]').map((row) => row.attributes("data-test"))
    ).toEqual(["model-row-my-model", "model-row-fast"]);

    await sortSelect.setValue("alias");

    expect(
      wrapper.findAll('[data-test^="model-row-"]').map((row) => row.attributes("data-test"))
    ).toEqual(["model-row-fast", "model-row-my-model"]);
  });

  it("shows a filtered empty state when no profiles match search", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="model-search-input"]').setValue("does-not-exist");

    const empty = wrapper.find('[data-test="model-filter-empty-state"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("No model profiles match your search.");
    expect(wrapper.find('[data-test="model-list"]').exists()).toBe(false);
  });

  it("renders source and effective audit state for model profiles", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    const audit = wrapper.find('[data-test="model-audit-my-model"]');
    expect(audit.exists()).toBe(true);
    expect(audit.text()).toContain("Source");
    expect(audit.text()).toContain("profiles.toml");
    expect(audit.text()).toContain("State");
    expect(audit.text()).toContain("Enabled");
    expect(audit.text()).toContain("Effective");
    expect(audit.text()).toContain("Active");
  });

  it("uses shared card content hierarchy for profile rows", () => {
    expectSourceMigration(modelProfileCardSource, {
      required: ["SettingsItemSummary", "SettingsStatusTag"],
      forbidden: [
        ".model-settings__profile-main",
        ".server__tags",
        "tag-success",
        "tag-warning",
        "tag--source"
      ]
    });
  });

  it("does not keep model pane aria chrome inline in the component source", () => {
    expectSourceMigration(modelSettingsPaneSource, {
      forbidden: ['aria-label="Model settings"']
    });
  });

  it("loads only the selected user configuration scope", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    expect(mockedCommands.listProfileSettings).toHaveBeenLastCalledWith("user");
    expect(mockedCommands.getEffectiveModelProfiles).not.toHaveBeenCalled();
    expect(wrapper.find('[data-test="model-row-local-code"]').exists()).toBe(false);
  });

  it("refresh button reloads profiles", async () => {
    const wrapper = mountPane("user");
    await flushPromises();
    expect(mockedCommands.listProfileSettings).toHaveBeenCalledTimes(1);

    await wrapper.find('[data-test="model-refresh"]').trigger("click");
    expect(mockedCommands.listProfileSettings).toHaveBeenCalledTimes(2);
  });

  it("add dialog opens, validates required fields, and saves", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="model-add-profile"]').trigger("click");
    await flushPromises();

    // Save with empty required fields should not call upsert
    await wrapper.find('[data-test="model-save-button"]').trigger("click");
    expect(mockedCommands.upsertProfileSettings).not.toHaveBeenCalled();

    // Fill required fields
    const aliasInput = wrapper.find('[data-test="model-form-alias"]');
    await aliasInput.setValue("new-model");
    await wrapper.find('[data-test="model-form-provider"]').setValue("ollama");
    await wrapper.find('[data-test="model-form-model-id"]').setValue("llama3");

    await wrapper.find('[data-test="model-save-button"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.upsertProfileSettings).toHaveBeenCalledWith(
      expect.objectContaining({
        alias: "new-model",
        provider: "ollama",
        model_id: "llama3",
        enabled: true
      })
    );
  });

  it("edit dialog opens pre-filled and saves changes", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="model-edit-my-model"]').trigger("click");
    await flushPromises();

    // Alias should be readonly with correct value
    const aliasInput = wrapper.find('[data-test="model-edit-alias"]');
    expect((aliasInput.element as HTMLInputElement).readOnly).toBe(true);

    // Change provider and save
    await wrapper.find('[data-test="model-edit-provider"]').setValue("anthropic");
    await wrapper.find('[data-test="model-edit-save-button"]').trigger("click");
    await flushPromises();

    expect(mockedCommands.upsertProfileSettings).toHaveBeenCalledWith(
      expect.objectContaining({ provider: "anthropic" })
    );
  });

  it("toggle button disables an enabled profile", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    const myModelRow = wrapper.find('[data-test="model-row-my-model"]');
    await myModelRow.find('[data-test="model-enable-my-model"]').trigger("click");

    expect(mockedCommands.setProfileEnabled).toHaveBeenCalledWith("my-model", false);
  });

  it("delete button only appears for writable profiles", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    expect(wrapper.find('[data-test="model-delete-my-model"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-delete-fast"]').exists()).toBe(false);
  });

  it("delete button removes profile", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="model-delete-my-model"]').trigger("click");
    expect(mockedCommands.deleteProfileSettings).toHaveBeenCalledWith("my-model");
  });

  it("move up/down buttons call moveProfileInOrder", async () => {
    const wrapper = mountPane("user");
    await flushPromises();

    // Verify rows are rendered first
    const myRow = wrapper.find('[data-test="model-row-my-model"]');
    expect(myRow.exists()).toBe(true);

    // Find move buttons within the rows
    const fastRow = wrapper.find('[data-test="model-row-fast"]');
    const fastUpBtn = fastRow.find('[data-test="model-move-up-fast"]');
    expect(fastUpBtn.exists()).toBe(true);

    const myDownBtn = myRow.find('[data-test="model-move-down-my-model"]');
    expect(myDownBtn.exists()).toBe(true);

    // "my-model" is at index 0 (not last), so the move-down button should be enabled
    await myDownBtn.trigger("click");
    expect(mockedCommands.moveProfileInOrder).toHaveBeenCalledWith("my-model", 1);
  });

  it("renders without ConfigSourceBar parent (defaults to user scope)", async () => {
    const wrapper = mountPane();
    await flushPromises();

    // When configSource is not provided (unit test context), defaults to null (user scope)
    expect(mockedCommands.listProfileSettings).toHaveBeenLastCalledWith(null);
    // Profile rows should still render
    expect(wrapper.find('[data-test="model-row-my-model"]').exists()).toBe(true);
  });

  it("shows error message on fetch failure", async () => {
    mockedCommands.listProfileSettings.mockRejectedValue(new Error("fetch failed"));
    const wrapper = mountPane("user");
    await flushPromises();

    const error = wrapper.find('[data-test="model-page-error"]');
    expect(error.exists()).toBe(true);
    expect(error.classes()).toContain("settings-state");
    expect(error.classes()).toContain("kx-state-block--error");
  });

  it("shows empty state when no profiles", async () => {
    mockedCommands.listProfileSettings.mockResolvedValue(ok([]));
    mockedCommands.getEffectiveModelProfiles.mockResolvedValue(ok([]));
    const wrapper = mountPane();
    await flushPromises();

    const empty = wrapper.find('[data-test="model-empty-state"]');
    expect(empty.exists()).toBe(true);
    expect(empty.classes()).toContain("settings-state");
    expect(empty.classes()).toContain("kx-state-block--empty");
    expect(empty.text()).toContain("No model profiles configured");
  });

  it("uses shared settings toolbar instead of local model toolbar chrome", () => {
    expectSourceMigration(modelSettingsPaneSource, {
      required: ["SettingsToolbar", "SettingsFilterBar"],
      forbidden: [
        'class="model-toolbar"',
        'class="model-toolbar__actions"',
        ".model-toolbar {",
        ".model-toolbar__actions {"
      ]
    });
  });
});
