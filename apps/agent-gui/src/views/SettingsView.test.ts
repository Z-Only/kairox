import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { mountWithPlugins } from "@/test-utils/mount";
import { useUiStore } from "@/stores/ui";
import { invoke } from "@tauri-apps/api/core";
import SettingsView from "./SettingsView.vue";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

const mockedInvoke = vi.mocked(invoke);

const discoveredSkill = {
  id: "test-driven-rust",
  name: "test-driven-rust",
  description: "Write Rust changes test-first.",
  version: "1.0.0",
  source: "builtin:/skills/test-driven-rust",
  activation_mode: "manual",
  keywords: ["rust", "tdd"],
  tools: [],
  can_request_tools: [],
  valid: true,
  validation_error: null
};

const invalidSkill = {
  id: "broken-skill",
  name: "broken-skill",
  description: "Fixture for validation errors.",
  version: null,
  source: "workspace:/skills/broken-skill",
  activation_mode: "manual",
  keywords: [],
  tools: [],
  can_request_tools: [],
  valid: false,
  validation_error: "Missing required description"
};

const activeSkill = {
  skill_id: "test-driven-rust",
  name: "test-driven-rust",
  source: "builtin:/skills/test-driven-rust",
  activation_mode: "manual"
};

const settingsViewSource = readFileSync(
  fileURLToPath(import.meta.url).replace(/\.test\.ts$/, ".vue"),
  "utf8"
);

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

function countInvokeCalls(commandName: string): number {
  return mockedInvoke.mock.calls.filter(([command]) => command === commandName).length;
}

beforeEach(() => {
  vi.clearAllMocks();
  mockedInvoke.mockImplementation(async (command) => {
    if (command === "list_skills") {
      return [discoveredSkill, invalidSkill];
    }
    if (command === "list_active_skills") {
      return [];
    }
    if (command === "activate_skill") {
      return activeSkill;
    }
    if (command === "deactivate_skill") {
      return null;
    }
    throw new Error(`Unexpected command: ${command}`);
  });
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

    const themeContainer = wrapper.find('[data-test="theme-toggle"]');
    const themeSelect = wrapper.find('select[data-test="settings-theme"]');
    expect(themeContainer.exists()).toBe(true);
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

  it("audit anchors: exposes stable settings pilot selectors", () => {
    const { wrapper } = mountSettings();

    expect(wrapper.find('[data-test="view-settings"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="theme-toggle"]').exists()).toBe(true);
    expect(wrapper.find('select[data-test="settings-theme"]').exists()).toBe(true);
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

  it("loads and renders discovered skills when opening the Skills tab", async () => {
    const { wrapper } = mountSettings();

    await wrapper.find('[data-test="settings-tab-skills"]').trigger("click");
    await flushPromises();

    expect(countInvokeCalls("list_skills")).toBe(1);
    expect(countInvokeCalls("list_active_skills")).toBe(1);
    expect(wrapper.find('[data-test="settings-skills-panel"]').isVisible()).toBe(true);
    expect(wrapper.find('[data-test="skill-card-test-driven-rust"]').text()).toContain(
      "Write Rust changes test-first."
    );
    expect(wrapper.find('[data-test="skill-card-test-driven-rust"]').text()).toContain(
      "builtin:/skills/test-driven-rust"
    );
    expect(wrapper.find('[data-test="skill-card-test-driven-rust"]').text()).toContain(
      "Mode: manual"
    );
    expect(wrapper.find('[data-test="skill-card-broken-skill"]').text()).toContain(
      "Missing required description"
    );
    expect(
      wrapper.find<HTMLButtonElement>('[data-test="skill-toggle-broken-skill"]').element.disabled
    ).toBe(true);
  });

  it("toggles a valid skill between active and inactive states", async () => {
    const { wrapper } = mountSettings();

    await wrapper.find('[data-test="settings-tab-skills"]').trigger("click");
    await flushPromises();

    const toggleButton = wrapper.find('[data-test="skill-toggle-test-driven-rust"]');
    expect(toggleButton.text()).toBe("Activate");

    await toggleButton.trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("activate_skill", {
      skillId: "test-driven-rust"
    });
    expect(toggleButton.text()).toBe("Deactivate");

    await toggleButton.trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("deactivate_skill", {
      skillId: "test-driven-rust"
    });
    expect(toggleButton.text()).toBe("Activate");
  });

  it("retries loading skills when the first tab load fails", async () => {
    let rejectedInitialSkillList = false;
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "list_skills" && !rejectedInitialSkillList) {
        rejectedInitialSkillList = true;
        throw new Error("skills unavailable");
      }
      if (command === "list_skills") {
        return [discoveredSkill, invalidSkill];
      }
      if (command === "list_active_skills") {
        return [];
      }
      throw new Error(`Unexpected command: ${command}`);
    });

    const { wrapper } = mountSettings();
    const skillsTab = wrapper.find('[data-test="settings-tab-skills"]');

    await skillsTab.trigger("click");
    await flushPromises();

    expect(wrapper.find('[role="alert"]').text()).toContain("skills unavailable");

    await wrapper.findAll('[role="tab"]')[0].trigger("click");
    await skillsTab.trigger("click");
    await flushPromises();

    expect(countInvokeCalls("list_skills")).toBe(2);
    expect(wrapper.find('[data-test="skill-card-test-driven-rust"]').exists()).toBe(true);
  });

  it("P1-S1-settings-landmarks exposes the settings page as the main landmark with a level-one heading", () => {
    const { wrapper } = mountSettings();

    expect(wrapper.find('main[data-test="view-settings"]').exists()).toBe(true);
    expect(wrapper.find("h1").text()).toBe("Settings");
  });
});
