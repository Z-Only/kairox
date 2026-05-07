import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { createTestingPinia } from "@pinia/testing";
// `createI18n` (and `createRouter` / `createMemoryHistory` in sibling
// view-level specs) are not part of `unplugin-auto-import`'s default
// `vue-i18n` / `vue-router` presets — those presets only expose the
// runtime hooks (`useI18n`, `useRoute`, `useRouter`). Test setup that
// instantiates a fresh i18n / router per spec must therefore keep
// these imports explicit.
import { createI18n } from "vue-i18n";
import en from "@/locales/en.json";
import { useUiStore } from "@/stores/ui";
import SettingsView from "./SettingsView.vue";

function makeI18n() {
  return createI18n({ legacy: false, locale: "en", messages: { en } });
}

function mountSettings() {
  const pinia = createTestingPinia({
    createSpy: vi.fn,
    stubActions: true,
    initialState: {
      ui: { locale: "en", colorMode: "auto" }
    }
  });
  const wrapper = mount(SettingsView, {
    global: { plugins: [pinia, makeI18n()] }
  });
  return { wrapper, ui: useUiStore(pinia) };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("SettingsView (Pre-work B regression)", () => {
  it("renders the locale select with the store value and routes writes through ui.setLocale", async () => {
    const { wrapper, ui } = mountSettings();

    const localeSelect = wrapper.find<HTMLSelectElement>(
      "select#settings-locale"
    );
    expect(localeSelect.exists()).toBe(true);
    expect(localeSelect.element.value).toBe("en");

    await localeSelect.setValue("zh-CN");

    expect(ui.setLocale).toHaveBeenCalledTimes(1);
    expect(ui.setLocale).toHaveBeenCalledWith("zh-CN");
    // The destructured ref must NOT have been mutated directly by a v-model
    // double-write (the store action is the single write path).
    expect(ui.locale).toBe("en");
  });

  it("renders the theme select with the store value and routes writes through ui.setTheme", async () => {
    const { wrapper, ui } = mountSettings();

    const themeSelect = wrapper.find<HTMLSelectElement>(
      "select#settings-theme"
    );
    expect(themeSelect.exists()).toBe(true);
    expect(themeSelect.element.value).toBe("auto");

    await themeSelect.setValue("dark");

    expect(ui.setTheme).toHaveBeenCalledTimes(1);
    expect(ui.setTheme).toHaveBeenCalledWith("dark");
    expect(ui.colorMode).toBe("auto");
  });

  it("pairs each <select> with a matching <label for=...> for accessibility", () => {
    const { wrapper } = mountSettings();
    const localeLabel = wrapper.find('label[for="settings-locale"]');
    const themeLabel = wrapper.find('label[for="settings-theme"]');
    expect(localeLabel.exists()).toBe(true);
    expect(themeLabel.exists()).toBe(true);
  });
});
