import { describe, it, expect, beforeEach, vi } from "vitest";
import { ref } from "vue";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import HooksSettingsPane from "./HooksSettingsPane.vue";
import hooksSettingsPaneSource from "./HooksSettingsPane.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

const hooksSettings = {
  user: [
    {
      id: "verify",
      event: "Stop",
      matcher: "*",
      command: "cargo test",
      statusMessage: "Running tests",
      timeoutSecs: 120,
      enabled: true,
      source: "User",
      configPath: "/home/.kairox/config.toml"
    },
    {
      id: "notify",
      event: "PreToolUse",
      matcher: "shell",
      command: "echo tool",
      statusMessage: "Preparing tool notification",
      timeoutSecs: 30,
      enabled: false,
      source: "User",
      configPath: "/home/.kairox/config.toml"
    }
  ],
  project: [],
  templates: [
    {
      id: "stop-validation",
      name: "Stop validation",
      description: "Run tests after a turn stops.",
      event: "Stop",
      matcher: "*",
      command: "cargo test --workspace --all-targets",
      statusMessage: "Running workspace validation",
      timeoutSecs: 600
    }
  ],
  userConfigPath: "/home/.kairox/config.toml",
  projectConfigPath: null
};

function mountPane(
  configSource: "user" | "project" = "user",
  configProjectId?: string,
  locale?: "en" | "zh-CN"
) {
  return mountWithPlugins(HooksSettingsPane, {
    locale,
    mount: {
      global: {
        provide: {
          configSource: ref(configSource),
          configProjectId: ref(configProjectId)
        }
      }
    },
    reusePinia: true
  }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("HooksSettingsPane", () => {
  it("shows loading with the shared state block", async () => {
    mockedInvoke.mockReturnValueOnce(new Promise(() => {}));

    const wrapper = mountPane("user");
    await flushPromises();

    const loading = wrapper.find('[data-test="hooks-loading"]');
    expect(loading.exists()).toBe(true);
    expect(loading.classes()).toContain("settings-state");
    expect(loading.classes()).toContain("kx-state-block--loading");
  });

  it("shows errors with the shared state block", async () => {
    mockedInvoke.mockRejectedValueOnce("hooks unavailable");

    const wrapper = mountPane("user");
    await flushPromises();

    const error = wrapper.find('[data-test="hooks-error"]');
    expect(error.exists()).toBe(true);
    expect(error.classes()).toContain("settings-state");
    expect(error.classes()).toContain("kx-state-block--error");
    expect(error.text()).toContain("hooks unavailable");
  });

  it("uses shared settings state chrome for an empty hook scope", async () => {
    mockedInvoke.mockResolvedValueOnce({ ...hooksSettings, user: [] });

    const wrapper = mountPane("user");
    await flushPromises();

    const empty = wrapper.find('[data-test="hooks-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.classes()).toContain("settings-state");
    expect(empty.text()).toContain("No hooks configured.");
  });

  it("opens the hook editor in a centered modal instead of the right grid column", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);

    const wrapper = mountPane("user");
    await flushPromises();

    expect(wrapper.find('[data-test="hook-form"]').exists()).toBe(false);

    await wrapper.find('[data-test="hook-add"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="hook-form"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="hook-editor-dialog"]').exists()).toBe(true);
    expect(wrapper.find(".kx-modal__panel").exists()).toBe(true);
    expectSourceMigration(hooksSettingsPaneSource, {
      forbidden: ["grid-template-columns: minmax(0, 1fr)"]
    });
  });

  it("loads and displays user hooks", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);

    const wrapper = mountPane("user");
    await flushPromises();

    expect(wrapper.find('[data-test="hooks-list"]').classes()).toContain("settings-card-list");
    expect(wrapper.find('[data-test="hook-search-input"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="hook-row-verify"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="hook-row-notify"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="hook-row-verify"]').classes()).toContain("settings-card-item");
    expect(wrapper.find('[data-test="hook-row-verify"]').text()).toContain("Stop");
    expect(wrapper.find('[data-test="hook-row-verify"]').text()).toContain("cargo test");
  });

  it("filters hooks by search text", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);

    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="hook-search-input"]').setValue("notify");

    expect(wrapper.find('[data-test="hook-row-notify"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="hook-row-verify"]').exists()).toBe(false);
  });

  it("matches hook search against metadata", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);

    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="hook-search-input"]').setValue("pretooluse");

    expect(wrapper.find('[data-test="hook-row-notify"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="hook-row-verify"]').exists()).toBe(false);
  });

  it("sorts hooks after filtering by search text", async () => {
    mockedInvoke.mockResolvedValueOnce({
      ...hooksSettings,
      user: [
        {
          ...hooksSettings.user[0],
          id: "zeta",
          event: "Stop",
          command: "shared matching command"
        },
        {
          ...hooksSettings.user[1],
          id: "alpha",
          event: "PreToolUse",
          command: "shared matching command"
        },
        {
          ...hooksSettings.user[0],
          id: "ignored",
          command: "unrelated command"
        }
      ]
    });

    const wrapper = mountPane("user");
    await flushPromises();

    const sortSelect = wrapper.find<HTMLSelectElement>('[data-test="hook-sort-select"]');
    expect(sortSelect.exists()).toBe(true);
    expect(sortSelect.attributes("aria-label")).toBe("Hook sort");

    await wrapper.find('[data-test="hook-search-input"]').setValue("matching");
    await sortSelect.setValue("id");

    const rowIds = wrapper
      .findAll('[data-test^="hook-row-"]')
      .map((row) => row.attributes("data-test"));
    expect(rowIds).toEqual(["hook-row-alpha", "hook-row-zeta"]);
  });

  it("localizes hook search and sort controls in Chinese", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);

    const wrapper = mountPane("user", undefined, "zh-CN");
    await flushPromises();

    expect(wrapper.get('[data-test="hook-search-input"]').attributes("placeholder")).toBe(
      "搜索钩子"
    );
    expect(wrapper.get('[data-test="hook-sort-select"]').text()).toContain("原始顺序");
  });

  it("shows a filtered empty state when no hooks match search", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);

    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="hook-search-input"]').setValue("does-not-exist");

    const empty = wrapper.find('[data-test="hooks-filter-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("No hooks match your search.");
    expect(wrapper.find('[data-test="hooks-list"]').exists()).toBe(false);
  });

  it("keeps the hook add action in the list header and lets the list use the full card width", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);

    const wrapper = mountPane("user");
    await flushPromises();

    const header = wrapper.find(".hooks-pane__list-header");
    expect(header.find('[data-test="hook-add"]').exists()).toBe(true);
    expect(wrapper.find('.hooks-pane__grid > [data-test="hook-add"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="hooks-list"]').classes()).toContain(
      "settings-card-list--auto-columns"
    );
    expectSourceMigration(hooksSettingsPaneSource, {
      required: ["width: 100%"],
      forbidden: ["width: min(100%, 760px)"],
      forbiddenPatterns: [/\.hooks-pane__list\s*\{[^}]*max-width:/]
    });
  });

  it("does not keep hook search and sort copy inline in the component source", () => {
    expectSourceMigration(hooksSettingsPaneSource, {
      forbidden: [
        'aria-label="Search hooks"',
        'placeholder="Search hooks"',
        "Original order",
        "No hooks match your search."
      ]
    });
  });

  it("saves the edited hook in user scope", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);
    mockedInvoke.mockResolvedValueOnce(null);
    mockedInvoke.mockResolvedValueOnce(hooksSettings);

    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="hook-edit-verify"]').trigger("click");
    await flushPromises();
    await wrapper.find<HTMLInputElement>('[data-test="hook-command"]').setValue("cargo fmt");
    await wrapper.find('[data-test="hook-save"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("upsert_hook_settings", {
      input: {
        scope: "User",
        id: "verify",
        event: "Stop",
        matcher: "*",
        command: "cargo fmt",
        statusMessage: "Running tests",
        timeoutSecs: 120,
        enabled: true
      },
      projectRoot: null
    });
  });

  it("deletes the selected hook", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);
    mockedInvoke.mockResolvedValueOnce(null);
    mockedInvoke.mockResolvedValueOnce({ ...hooksSettings, user: [] });

    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="hook-delete-verify"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("delete_hook_settings", {
      scope: "User",
      event: "Stop",
      id: "verify",
      projectRoot: null
    });
  });

  it("fills the form from a built-in template", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);

    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="hook-template-stop-validation"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="hook-editor-dialog"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="hook-form"]').exists()).toBe(true);
    expect(wrapper.find<HTMLInputElement>('[data-test="hook-id"]').element.value).toBe(
      "stop-validation"
    );
    expect(wrapper.find<HTMLInputElement>('[data-test="hook-command"]').element.value).toContain(
      "cargo test --workspace"
    );
  });

  it("does not keep local hook row chrome after moving to SettingsCardItem", () => {
    expectSourceMigration(hooksSettingsPaneSource, {
      required: [
        "SettingsFilterBar",
        "SettingsCardList",
        "SettingsCardItem",
        "SettingsItemSummary",
        "SettingsStatusTag"
      ],
      forbidden: [
        ".hook-row {",
        ".hook-row__main",
        "tag-muted",
        "border-bottom: 1px solid var(--app-border-color)"
      ]
    });
  });

  it("uses shared form fields, controls, and action rows in the hook editor", () => {
    expectSourceMigration(hooksSettingsPaneSource, {
      required: ["KxFormField", "KxFormActions", "KxInput", "KxSelect"],
      forbidden: [
        "kx-form-control",
        ".hooks-pane__form input,",
        ".hooks-pane__form select",
        ".hooks-pane__form-actions {"
      ]
    });
  });

  it("does not keep hook pane aria chrome inline in the component source", () => {
    expectSourceMigration(hooksSettingsPaneSource, {
      forbidden: ['aria-label="Hooks settings"']
    });
  });
});
