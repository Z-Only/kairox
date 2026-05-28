import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import SettingsFilterBar from "./SettingsFilterBar.vue";

describe("SettingsFilterBar", () => {
  it("renders with the settings-filter-bar class", () => {
    const wrapper = mount(SettingsFilterBar);

    expect(wrapper.classes()).toContain("settings-filter-bar");
  });

  it("forwards ariaLabel and dataTest props to the root element", () => {
    const wrapper = mount(SettingsFilterBar, {
      props: {
        ariaLabel: "Filter models",
        dataTest: "model-filters"
      }
    });

    expect(wrapper.attributes("aria-label")).toBe("Filter models");
    expect(wrapper.attributes("data-test")).toBe("model-filters");
  });

  it("omits optional attributes when props are not provided", () => {
    const wrapper = mount(SettingsFilterBar);

    expect(wrapper.attributes("aria-label")).toBeUndefined();
    expect(wrapper.attributes("data-test")).toBeUndefined();
  });

  it("renders slot content inside the filter bar", () => {
    const wrapper = mount(SettingsFilterBar, {
      slots: {
        default: '<input type="search" data-test="q" /><button>Go</button>'
      }
    });

    expect(wrapper.get('[data-test="q"]').attributes("type")).toBe("search");
    expect(wrapper.find("button").text()).toBe("Go");
  });

  it("renders as a div element (no implicit ARIA role)", () => {
    const wrapper = mount(SettingsFilterBar);

    expect(wrapper.element.tagName).toBe("DIV");
    expect(wrapper.attributes("role")).toBeUndefined();
  });
});
