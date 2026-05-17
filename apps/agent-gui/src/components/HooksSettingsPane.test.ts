import { describe, it, expect, beforeEach, vi } from "vitest";
import { ref } from "vue";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import HooksSettingsPane from "./HooksSettingsPane.vue";
import { mountWithPlugins } from "@/test-utils/mount";

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

function mountPane(configSource: "user" | "project" = "user", configProjectId?: string) {
  return mountWithPlugins(HooksSettingsPane, {
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
  it("loads and displays user hooks", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);

    const wrapper = mountPane("user");
    await flushPromises();

    expect(wrapper.find('[data-test="hook-row-verify"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="hook-row-verify"]').text()).toContain("Stop");
    expect(wrapper.find('[data-test="hook-row-verify"]').text()).toContain("cargo test");
  });

  it("saves the edited hook in user scope", async () => {
    mockedInvoke.mockResolvedValueOnce(hooksSettings);
    mockedInvoke.mockResolvedValueOnce(null);
    mockedInvoke.mockResolvedValueOnce(hooksSettings);

    const wrapper = mountPane("user");
    await flushPromises();

    await wrapper.find('[data-test="hook-edit-verify"]').trigger("click");
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

    expect(wrapper.find<HTMLInputElement>('[data-test="hook-id"]').element.value).toBe(
      "stop-validation"
    );
    expect(wrapper.find<HTMLInputElement>('[data-test="hook-command"]').element.value).toContain(
      "cargo test --workspace"
    );
  });
});
