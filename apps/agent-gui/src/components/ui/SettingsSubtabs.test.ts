import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import SettingsSubtabs from "./SettingsSubtabs.vue";

describe("SettingsSubtabs", () => {
  it("renders with the settings-subtabs class and tablist role", () => {
    const wrapper = mount(SettingsSubtabs);

    expect(wrapper.classes()).toContain("settings-subtabs");
    expect(wrapper.attributes("role")).toBe("tablist");
  });

  it("forwards ariaLabel and dataTest props to the root element", () => {
    const wrapper = mount(SettingsSubtabs, {
      props: {
        ariaLabel: "Settings sections",
        dataTest: "subtab-bar"
      }
    });

    expect(wrapper.attributes("aria-label")).toBe("Settings sections");
    expect(wrapper.attributes("data-test")).toBe("subtab-bar");
  });

  it("omits optional attributes when props are not provided", () => {
    const wrapper = mount(SettingsSubtabs);

    expect(wrapper.attributes("aria-label")).toBeUndefined();
    expect(wrapper.attributes("data-test")).toBeUndefined();
  });

  it("renders tab buttons passed via the default slot", () => {
    const wrapper = mount(SettingsSubtabs, {
      slots: {
        default: [
          '<button class="sub-tab-btn" role="tab" aria-selected="true" data-test="tab-general">General</button>',
          '<button class="sub-tab-btn" role="tab" aria-selected="false" data-test="tab-advanced">Advanced</button>'
        ].join("")
      }
    });

    const tabs = wrapper.findAll('[role="tab"]');
    expect(tabs).toHaveLength(2);
    expect(wrapper.get('[data-test="tab-general"]').attributes("aria-selected")).toBe("true");
    expect(wrapper.get('[data-test="tab-advanced"]').attributes("aria-selected")).toBe("false");
  });

  it("renders as a div element with explicit tablist role", () => {
    const wrapper = mount(SettingsSubtabs);

    expect(wrapper.element.tagName).toBe("DIV");
    expect(wrapper.attributes("role")).toBe("tablist");
  });
});
