import { describe, expect, it } from "vitest";
import GeneralSettings from "./GeneralSettings.vue";
import generalSettingsSource from "./GeneralSettings.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";

describe("GeneralSettings", () => {
  it("keeps locale and theme selects compact instead of stretching across the row", () => {
    const wrapper = mountWithPlugins(GeneralSettings);

    expect(wrapper.find('[data-test="settings-locale"]').classes()).toContain("settings__select");
    expect(wrapper.find('[data-test="settings-theme"]').classes()).toContain("settings__select");
    expect(generalSettingsSource).toContain("max-width: 220px");
    expect(generalSettingsSource).toContain("flex: 0 1 220px");
  });
});
