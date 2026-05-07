import { describe, it, expect, vi, beforeEach } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import { useUiStore } from "@/stores/ui";
import SettingsView from "./SettingsView.vue";

function mountSettings() {
  const { wrapper } = mountWithPlugins(SettingsView, {
    withNaiveProviders: true,
    mount: {
      global: {
        stubs: {
          RouterView: true
        }
      }
    }
  });
  const ui = useUiStore();
  return { wrapper, ui };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("SettingsView (Pre-work B regression)", () => {
  it("renders the locale NSelect with the store value and routes writes through ui.setLocale", async () => {
    const { wrapper, ui } = mountSettings();

    const localeSelect = wrapper.find('[data-test="settings-locale"]');
    expect(localeSelect.exists()).toBe(true);
    expect(ui.locale).toBe("en");

    await ui.setLocale("zh-CN");

    expect(ui.setLocale).toHaveBeenCalledTimes(1);
    expect(ui.setLocale).toHaveBeenCalledWith("zh-CN");
  });

  it("renders the theme NSelect with the store value and routes writes through ui.setTheme", async () => {
    const { wrapper, ui } = mountSettings();

    const themeSelect = wrapper.find('[data-test="settings-theme"]');
    expect(themeSelect.exists()).toBe(true);
    expect(ui.colorMode).toBe("auto");

    await ui.setTheme("dark");

    expect(ui.setTheme).toHaveBeenCalledTimes(1);
    expect(ui.setTheme).toHaveBeenCalledWith("dark");
  });

  it("renders NTabs with General and Marketplace panes", () => {
    const { wrapper } = mountSettings();
    const tabs = wrapper.findComponent({ name: "NTabs" });
    expect(tabs.exists()).toBe(true);
    const tabPanes = wrapper.findAllComponents({ name: "NTabPane" });
    expect(tabPanes.length).toBe(2);
  });
});
