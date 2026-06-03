import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import GeneralSettings from "./GeneralSettings.vue";
import generalSettingsSource from "./GeneralSettings.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import { useUiStore } from "@/stores/ui";

vi.mock("@/generated/commands", () => ({
  commands: {
    getGuiSettings: vi.fn(),
    setGuiDevtoolsEnabled: vi.fn()
  }
}));

import { commands } from "@/generated/commands";

const mockedCommands = vi.mocked(
  commands as unknown as {
    getGuiSettings: () => Promise<{ status: "ok"; data: GuiSettingsFixture }>;
    setGuiDevtoolsEnabled: (
      enabled: boolean
    ) => Promise<{ status: "ok"; data: GuiSettingsFixture }>;
  }
);

interface GuiSettingsFixture {
  devtools_enabled: boolean;
  default_devtools_enabled: boolean;
  requires_restart: boolean;
}

describe("GeneralSettings", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockedCommands.getGuiSettings.mockResolvedValue(
      ok({
        devtools_enabled: false,
        default_devtools_enabled: false,
        requires_restart: false
      })
    );
    mockedCommands.setGuiDevtoolsEnabled.mockResolvedValue(
      ok({
        devtools_enabled: true,
        default_devtools_enabled: false,
        requires_restart: true
      })
    );
  });

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

  it("defaults the language preference to System when no stored locale exists", async () => {
    localStorage.clear();
    const { wrapper } = mountWithPlugins(GeneralSettings, {
      reusePinia: false
    });
    const ui = useUiStore();
    await flushPromises();

    expect(ui.locale).toBe("system");
    expect(wrapper.find<HTMLSelectElement>('[data-test="settings-locale"]').element.value).toBe(
      "system"
    );
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

  it("loads the developer tools advanced setting from the backend", async () => {
    const { wrapper } = mountWithPlugins(GeneralSettings, {
      reusePinia: false
    });
    await flushPromises();

    expect(mockedCommands.getGuiSettings).toHaveBeenCalledTimes(1);
    expect(wrapper.find('[data-test="settings-devtools"]').exists()).toBe(true);
    expect(wrapper.find<HTMLInputElement>('[data-test="settings-devtools"]').element.checked).toBe(
      false
    );
    expect(wrapper.text()).not.toContain("Restart required");
  });

  it("persists developer tools toggle through the backend command", async () => {
    const { wrapper } = mountWithPlugins(GeneralSettings, {
      reusePinia: false
    });
    await flushPromises();

    await wrapper.find('[data-test="settings-devtools"]').setValue(true);
    await flushPromises();

    expect(mockedCommands.setGuiDevtoolsEnabled).toHaveBeenCalledWith(true);
    expect(wrapper.find<HTMLInputElement>('[data-test="settings-devtools"]').element.checked).toBe(
      true
    );
    expect(wrapper.text()).toContain("Restart required");
  });
});

function ok<T>(data: T) {
  return { status: "ok" as const, data };
}
