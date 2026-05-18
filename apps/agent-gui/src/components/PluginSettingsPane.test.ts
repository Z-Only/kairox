import { describe, it, expect, beforeEach, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import PluginSettingsPane from "./PluginSettingsPane.vue";
import { mountWithPlugins } from "@/test-utils/mount";

vi.mock("@/generated/commands", () => ({
  commands: {
    listPluginSettings: vi.fn(),
    listPluginMarketplaceSources: vi.fn(),
    listPluginCatalog: vi.fn(),
    setPluginMarketplaceSourceEnabled: vi.fn(),
    setPluginEnabled: vi.fn(),
    deletePluginSettings: vi.fn(),
    installPlugin: vi.fn()
  }
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { commands } from "@/generated/commands";
const mockedCommands = vi.mocked(commands);

function ok<T>(data: T): { status: "ok"; data: T } {
  return { status: "ok", data };
}

function mountPane() {
  return mountWithPlugins(PluginSettingsPane, { reusePinia: true }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
  mockedCommands.listPluginSettings.mockResolvedValue(ok([]));
  mockedCommands.listPluginMarketplaceSources.mockResolvedValue(ok([]));
  mockedCommands.listPluginCatalog.mockResolvedValue(ok([]));
});

describe("PluginSettingsPane", () => {
  it("uses a single marketplace tab with source settings inside it", async () => {
    const wrapper = mountPane();
    await flushPromises();

    expect(wrapper.find('[data-test="plugin-subtab-installed"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="plugin-subtab-marketplace"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="plugin-subtab-discover"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="plugin-subtab-marketplaces"]').exists()).toBe(false);

    await wrapper.find('[data-test="plugin-subtab-marketplace"]').trigger("click");
    await flushPromises();

    expect(wrapper.find('[data-test="plugin-source-settings-toggle"]').exists()).toBe(true);
  });
});
