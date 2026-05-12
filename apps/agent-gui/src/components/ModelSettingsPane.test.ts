import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins, type MountWithPluginsOptions } from "@/test-utils/mount";
import { commands } from "@/generated/commands";
import ModelSettingsPane from "./ModelSettingsPane.vue";

vi.mock("@/generated/commands", () => ({
  commands: {
    listProfileSettings: vi.fn(),
    upsertProfileSettings: vi.fn(),
    setProfileEnabled: vi.fn(),
    deleteProfileSettings: vi.fn(),
    moveProfileInOrder: vi.fn(),
    openConfigDir: vi.fn()
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

const disabledProfile = {
  alias: "slow-model",
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
  source: "profiles_toml"
};

function ok<T>(data: T): { status: "ok"; data: T } {
  return { status: "ok", data };
}

function mountPane() {
  const mountOptions: MountWithPluginsOptions<typeof ModelSettingsPane> = {
    reusePinia: true
  };
  return mountWithPlugins(ModelSettingsPane, mountOptions).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockedCommands.listProfileSettings.mockResolvedValue(
    ok([writableProfile, readOnlyProfile, disabledProfile])
  );
  mockedCommands.upsertProfileSettings.mockResolvedValue(ok(writableProfile));
  mockedCommands.setProfileEnabled.mockResolvedValue(ok(null));
  mockedCommands.deleteProfileSettings.mockResolvedValue(ok(null));
  mockedCommands.moveProfileInOrder.mockResolvedValue(ok(null));
  mockedCommands.openConfigDir.mockResolvedValue(ok("/tmp/config"));
});

describe("ModelSettingsPane", () => {
  it("renders profile list with correct data", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="model-row-my-model"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-row-fast"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-row-slow-model"]').exists()).toBe(true);

    const myModelRow = wrapper.find('[data-test="model-row-my-model"]');
    expect(myModelRow.text()).toContain("Enabled");
    expect(myModelRow.text()).toContain("Has API Key");

    const slowRow = wrapper.find('[data-test="model-row-slow-model"]');
    expect(slowRow.text()).toContain("Disabled");
  });

  it("refresh button reloads profiles", async () => {
    const wrapper = mountPane();
    await flushPromises();
    expect(mockedCommands.listProfileSettings).toHaveBeenCalledTimes(1);

    await wrapper.find('[data-test="model-refresh"]').trigger("click");
    expect(mockedCommands.listProfileSettings).toHaveBeenCalledTimes(2);
  });

  it("add dialog opens, validates required fields, and saves", async () => {
    const wrapper = mountPane();
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
    const wrapper = mountPane();
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
    const wrapper = mountPane();
    await flushPromises();

    const myModelRow = wrapper.find('[data-test="model-row-my-model"]');
    await myModelRow.find('[data-test="model-enable-my-model"]').trigger("click");

    expect(mockedCommands.setProfileEnabled).toHaveBeenCalledWith("my-model", false);
  });

  it("delete button only appears for writable profiles", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="model-delete-my-model"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="model-delete-fast"]').exists()).toBe(false);
  });

  it("delete button removes profile", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="model-delete-my-model"]').trigger("click");
    expect(mockedCommands.deleteProfileSettings).toHaveBeenCalledWith("my-model");
  });

  it("move up/down buttons call moveProfileInOrder", async () => {
    const wrapper = mountPane();
    await flushPromises();

    // Verify rows are rendered first
    const fastRow = wrapper.find('[data-test="model-row-fast"]');
    expect(fastRow.exists()).toBe(true);
    const myRow = wrapper.find('[data-test="model-row-my-model"]');
    expect(myRow.exists()).toBe(true);

    // Find move buttons within the rows
    const fastUpBtn = fastRow.find('[data-test="model-move-up-fast"]');
    expect(fastUpBtn.exists()).toBe(true);

    const myDownBtn = myRow.find('[data-test="model-move-down-my-model"]');
    expect(myDownBtn.exists()).toBe(true);

    // "fast" is at index 1, so the move-up button should be enabled
    await fastUpBtn.trigger("click");
    expect(mockedCommands.moveProfileInOrder).toHaveBeenCalledWith("fast", -1);

    // "my-model" is at index 0 (not last), so the move-down button should be enabled
    await myDownBtn.trigger("click");
    expect(mockedCommands.moveProfileInOrder).toHaveBeenCalledWith("my-model", 1);
  });

  it("source toggle switches between user and project views", async () => {
    const wrapper = mountPane();
    await flushPromises();

    const select = wrapper.find('[data-test="model-source-filter"]');
    expect(select.exists()).toBe(true);

    await select.setValue("project");
    await flushPromises();
    expect(mockedCommands.listProfileSettings).toHaveBeenCalledWith("project");
  });

  it("open config dir button calls openConfigDir", async () => {
    const wrapper = mountPane();
    await flushPromises();

    await wrapper.find('[data-test="model-open-config-dir"]').trigger("click");
    expect(mockedCommands.openConfigDir).toHaveBeenCalled();
  });

  it("shows error message on fetch failure", async () => {
    mockedCommands.listProfileSettings.mockRejectedValue(new Error("fetch failed"));
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="model-page-error"]').exists()).toBe(true);
  });

  it("shows empty state when no profiles", async () => {
    mockedCommands.listProfileSettings.mockResolvedValue(ok([]));
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.text()).toContain("No model profiles configured");
  });
});
