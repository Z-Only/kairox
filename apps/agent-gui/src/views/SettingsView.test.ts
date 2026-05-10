import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, it, expect } from "vitest";
import { mountWithPlugins } from "@/test-utils/mount";
import { useUiStore } from "@/stores/ui";
import SettingsView from "./SettingsView.vue";
const settingsViewSource = readFileSync(
  fileURLToPath(import.meta.url).replace(/\.test\.ts$/, ".vue"),
  "utf8"
);

function mountSettings() {
  const { wrapper } = mountWithPlugins(SettingsView, {
    mount: {
      global: {
        stubs: {
          McpSettingsPane: {
            template: '<section data-test="settings-mcp-pane-stub">MCP settings pane</section>'
          },
          MarketplacePane: {
            template:
              '<section data-test="settings-marketplace-pane-stub">Marketplace pane</section>'
          },
          RouterView: true,
          SkillSettingsPane: {
            template: '<section data-test="settings-skill-pane-stub">Skills settings pane</section>'
          }
        }
      }
    }
  });
  const ui = useUiStore();
  return { wrapper, ui };
}

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

    const themeContainer = wrapper.find('[data-test="theme-toggle"]');
    const themeSelect = wrapper.find('select[data-test="settings-theme"]');
    expect(themeContainer.exists()).toBe(true);
    expect(themeSelect.exists()).toBe(true);
    expect(ui.colorMode).toBe("auto");

    await ui.setTheme("dark");

    expect(ui.colorMode).toBe("dark");
    expect(ui.isDark).toBe(true);
  });

  it("shows General, MCP, Skills, and Marketplace tabs", () => {
    const { wrapper } = mountSettings();

    const tabs = wrapper.findAll('[role="tab"]').map((tab) => tab.text());
    expect(tabs).toContain("General");
    expect(tabs).toContain("MCP");
    expect(tabs).toContain("Skills");
    expect(tabs).toContain("Marketplace");
  });

  it("audit anchors: exposes stable settings pilot selectors", () => {
    const { wrapper } = mountSettings();

    expect(wrapper.find('[data-test="view-settings"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="theme-toggle"]').exists()).toBe(true);
    expect(wrapper.find('select[data-test="settings-theme"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="settings-tab-mcp"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="settings-tab-skills"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="settings-tab-marketplace"]').exists()).toBe(true);
  });

  it("P1-S1-settings-tab-contrast keeps inactive tabs on accessible theme text color", () => {
    expect(settingsViewSource).toContain("color: var(--app-text-color-2, #6b7280);");
  });

  it("P2-S1-focus-ring exposes a visible focus indicator on the native theme control", async () => {
    const { wrapper } = mountSettings();
    const themeToggle = wrapper.find('[data-test="theme-toggle"]');
    const themeSelect = wrapper.find('select[data-test="settings-theme"]');

    expect(themeToggle.exists()).toBe(true);
    expect(themeSelect.exists()).toBe(true);
    expect(settingsViewSource).toContain('data-test="theme-toggle"');
    expect(settingsViewSource).toContain('data-test="settings-theme"');
    expect(settingsViewSource).toContain("settings__select--focused");
    expect(settingsViewSource).toContain("outline: 2px solid var(--app-primary-color, #3b82f6);");
    expect(settingsViewSource).toContain(
      "box-shadow: inset 0 0 0 2px var(--app-primary-color, #3b82f6);"
    );
    expect(settingsViewSource).toContain(
      "background-color: color-mix(in srgb, var(--app-primary-color, #3b82f6) 12%, transparent);"
    );

    await themeSelect.trigger("focus");
    expect(themeSelect.classes()).toContain("settings__select--focused");

    await themeSelect.trigger("blur");
    expect(themeSelect.classes()).not.toContain("settings__select--focused");
  });

  it("mounts the MCP settings pane from the MCP tab", async () => {
    const { wrapper } = mountSettings();

    await wrapper.find('[data-test="settings-tab-mcp"]').trigger("click");

    expect(wrapper.find('[data-test="settings-mcp-pane-stub"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="settings-skill-pane-stub"]').exists()).toBe(false);
  });

  it("mounts the Skills settings pane from the Skills tab", async () => {
    const { wrapper } = mountSettings();

    await wrapper.find('[data-test="settings-tab-skills"]').trigger("click");

    expect(wrapper.find('[data-test="settings-skill-pane-stub"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="settings-mcp-pane-stub"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="settings-marketplace-pane-stub"]').exists()).toBe(false);
  });

  it("mounts the Marketplace pane from the Marketplace tab", async () => {
    const { wrapper } = mountSettings();

    await wrapper.find('[data-test="settings-tab-marketplace"]').trigger("click");

    expect(wrapper.find('[data-test="settings-marketplace-pane-stub"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="settings-mcp-pane-stub"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="settings-skill-pane-stub"]').exists()).toBe(false);
  });

  it("P1-S1-settings-landmarks exposes the settings page as the main landmark with a level-one heading", () => {
    const { wrapper } = mountSettings();

    expect(wrapper.find('main[data-test="view-settings"]').exists()).toBe(true);
    expect(wrapper.find("h1").text()).toBe("Settings");
  });
});
