import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SettingsState from "./SettingsState.vue";

describe("SettingsState", () => {
  it("wraps settings empty states with stable chrome and selectors", () => {
    const wrapper = mount(SettingsState, {
      props: { tone: "empty", dataTest: "settings-empty" },
      slots: { default: "No settings yet" }
    });

    expect(wrapper.attributes("data-test")).toBe("settings-empty");
    expect(wrapper.classes()).toContain("settings-state");
    expect(wrapper.classes()).toContain("settings-state--empty");
    expect(wrapper.find(".settings-state__message").text()).toBe("No settings yet");
  });

  it("keeps error and loading state roles accessible", () => {
    const error = mount(SettingsState, {
      props: { tone: "error" },
      slots: { default: "Failed to load settings" }
    });
    const loading = mount(SettingsState, {
      props: { tone: "loading" },
      slots: { default: "Loading settings" }
    });

    expect(error.attributes("role")).toBe("alert");
    expect(loading.attributes("role")).toBe("status");
  });

  it("supports a recovery action slot without changing the message", () => {
    const wrapper = mount(SettingsState, {
      props: { tone: "error", dataTest: "settings-error" },
      slots: {
        default: "Could not load settings",
        actions: '<button type="button">Retry</button>'
      }
    });

    expect(wrapper.find(".settings-state__message").text()).toBe("Could not load settings");
    expect(wrapper.find(".settings-state__actions button").text()).toBe("Retry");
  });
});
