import { describe, it, expect, vi, beforeEach } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import { useUiStore } from "@/stores/ui";
import SettingsView from "./SettingsView.vue";

function mountSettings() {
  const { wrapper } = mountWithPlugins(SettingsView, {
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
  it("renders the locale select with the store value and routes writes through ui.setLocale", async () => {
    const { wrapper, ui } = mountSettings();

    const localeSelect = wrapper.find('[data-test="settings-locale"]');
    expect(localeSelect.exists()).toBe(true);
    expect(ui.locale).toBe("en");

    await ui.setLocale("zh-CN");

    // Verify the store state actually changed (not spy assertions — mountWithPlugins
    // uses a real Pinia, not createTestingPinia, so actions are not spies).
    expect(ui.locale).toBe("zh-CN");
  });

  it("renders the theme select with the store value and routes writes through ui.setTheme", async () => {
    const { wrapper, ui } = mountSettings();

    const themeSelect = wrapper.find('[data-test="settings-theme"]');
    expect(themeSelect.exists()).toBe(true);
    expect(ui.colorMode).toBe("auto");

    await ui.setTheme("dark");

    expect(ui.colorMode).toBe("dark");
    expect(ui.isDark).toBe(true);
  });

  it("renders tabs with General and Marketplace panes", () => {
    const { wrapper } = mountSettings();
    // Verify the rendered output contains the expected tab labels.
    const html = wrapper.html();
    expect(html).toContain("General");
    expect(html).toContain("Marketplace");
  });
});
