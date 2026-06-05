import { beforeEach, describe, expect, it, vi } from "vitest";
import { flushPromises } from "@vue/test-utils";
import GeneralSettings from "./GeneralSettings.vue";
import generalSettingsSource from "./GeneralSettings.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import { useUiStore } from "@/stores/ui";
import {
  updateAvailable,
  updateInfo,
  lastCheckTime,
  lastCheckError,
  checkingForUpdate,
  downloadingUpdate
} from "@/composables/useUpdater";

vi.mock("@/generated/commands", () => ({
  commands: {
    getGuiSettings: vi.fn(),
    setGuiDevtoolsEnabled: vi.fn()
  }
}));

const mockDownloadAndInstall = vi.fn().mockResolvedValue(undefined);
const mockCheck = vi.fn().mockResolvedValue(null);
vi.mock("@tauri-apps/plugin-updater", () => ({
  check: (...args: unknown[]) => mockCheck(...args)
}));

vi.mock("@tauri-apps/plugin-process", () => ({
  relaunch: vi.fn()
}));

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn().mockResolvedValue("0.37.0")
}));

vi.mock("@/locales", () => ({
  i18n: {
    global: {
      t: (key: string) => key
    }
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
    updateAvailable.value = false;
    updateInfo.value = null;
    lastCheckTime.value = null;
    lastCheckError.value = null;
    checkingForUpdate.value = false;
    downloadingUpdate.value = false;
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

  it("renders the software update section with version badge", async () => {
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    expect(wrapper.find('[data-test="settings-current-version"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="settings-current-version"]').text()).toContain("0.37.0");
  });

  it("renders auto-check toggle defaulting to checked", async () => {
    localStorage.clear();
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    const autoCheck = wrapper.find<HTMLInputElement>('[data-test="settings-auto-check"]');
    expect(autoCheck.exists()).toBe(true);
    expect(autoCheck.element.checked).toBe(true);
  });

  it("shows check interval selector when auto-check is enabled", async () => {
    localStorage.clear();
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    expect(wrapper.find('[data-test="settings-check-interval"]').exists()).toBe(true);
  });

  it("hides check interval selector when auto-check is disabled", async () => {
    localStorage.clear();
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    await wrapper.find('[data-test="settings-auto-check"]').setValue(false);
    await flushPromises();

    expect(wrapper.find('[data-test="settings-check-interval"]').exists()).toBe(false);
  });

  it("renders auto-download toggle defaulting to unchecked", async () => {
    localStorage.clear();
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    const autoDownload = wrapper.find<HTMLInputElement>('[data-test="settings-auto-download"]');
    expect(autoDownload.exists()).toBe(true);
    expect(autoDownload.element.checked).toBe(false);
  });

  it("renders the check now button", async () => {
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    expect(wrapper.find('[data-test="settings-check-update"]').exists()).toBe(true);
  });

  it("triggers checkForUpdate when check now button is clicked", async () => {
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    await wrapper.find('[data-test="settings-check-update"]').trigger("click");
    await flushPromises();

    expect(mockCheck).toHaveBeenCalled();
  });

  it("triggers downloadAndInstallUpdate when download button is clicked", async () => {
    mockCheck.mockResolvedValue({
      version: "2.0.0",
      body: null,
      downloadAndInstall: mockDownloadAndInstall
    });
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    updateAvailable.value = true;
    updateInfo.value = { version: "2.0.0" };
    await flushPromises();

    const downloadBtn = wrapper.find('[data-test="settings-download-update"]');
    expect(downloadBtn.exists()).toBe(true);
    await downloadBtn.trigger("click");
    await flushPromises();

    expect(mockDownloadAndInstall).toHaveBeenCalled();
  });

  it("shows update available tag when update exists", async () => {
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    updateAvailable.value = true;
    updateInfo.value = { version: "2.0.0" };
    await flushPromises();

    expect(wrapper.find('[data-test="settings-update-available-tag"]').exists()).toBe(true);
  });

  it("shows up-to-date tag after a successful check with no update", async () => {
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    updateAvailable.value = false;
    lastCheckTime.value = Date.now();
    await flushPromises();

    expect(wrapper.find('[data-test="settings-up-to-date-tag"]').exists()).toBe(true);
  });

  it("shows last check time when available", async () => {
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    lastCheckTime.value = Date.now();
    await flushPromises();

    expect(wrapper.find('[data-test="settings-update-actions"]').text()).toContain("Last checked");
  });

  it("shows error message when last check failed", async () => {
    const { wrapper } = mountWithPlugins(GeneralSettings, { reusePinia: false });
    await flushPromises();

    lastCheckError.value = "Network error";
    await flushPromises();

    expect(wrapper.find('[data-test="settings-update-error"]').exists()).toBe(true);
  });
});

function ok<T>(data: T) {
  return { status: "ok" as const, data };
}
