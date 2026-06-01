import { describe, expect, it } from "vitest";
import { flushPromises } from "@vue/test-utils";
import GeneralSettings from "./GeneralSettings.vue";
import generalSettingsSource from "./GeneralSettings.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import { useUiStore } from "@/stores/ui";

describe("GeneralSettings", () => {
  it("keeps locale and theme selects compact instead of stretching across the row", () => {
    const wrapper = mountWithPlugins(GeneralSettings);

    expect(wrapper.find('[data-test="settings-locale"]').classes()).toContain("settings__select");
    expect(wrapper.find('[data-test="settings-theme"]').classes()).toContain("settings__select");
    expectSourceMigration(generalSettingsSource, {
      required: ["max-width: 160px", "flex: 0 1 160px", "text-align: center"]
    });
  });

  it("toggles settings__select--focused class on theme select focus/blur", async () => {
    const wrapper = mountWithPlugins(GeneralSettings);

    const themeSelect = wrapper.find('[data-test="settings-theme"]');
    expect(themeSelect.classes()).not.toContain("settings__select--focused");

    await themeSelect.trigger("focus");
    expect(themeSelect.classes()).toContain("settings__select--focused");

    await themeSelect.trigger("blur");
    expect(themeSelect.classes()).not.toContain("settings__select--focused");
  });

  it("calls setLocale on the ui store when locale select changes", async () => {
    const { wrapper } = mountWithPlugins(GeneralSettings, {
      reusePinia: false
    });
    const ui = useUiStore();
    await flushPromises();

    const localeSelect = wrapper.find('[data-test="settings-locale"]');
    // Simulate selecting "zh-CN"
    await localeSelect.setValue("zh-CN");
    await flushPromises();

    expect(ui.locale).toBe("zh-CN");
  });

  it("calls setTheme on the ui store when theme select changes", async () => {
    const { wrapper } = mountWithPlugins(GeneralSettings, {
      reusePinia: false
    });
    const ui = useUiStore();
    await flushPromises();

    const themeSelect = wrapper.find('[data-test="settings-theme"]');
    // Simulate selecting "dark"
    await themeSelect.setValue("dark");
    await flushPromises();

    expect(ui.colorMode).toBe("dark");
  });
});
