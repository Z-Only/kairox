import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxChipGroup from "./KxChipGroup.vue";

describe("KxChipGroup", () => {
  it("lays out chips and trailing actions through a shared source-filter primitive", () => {
    const wrapper = mount(KxChipGroup, {
      props: {
        ariaLabel: "Sources",
        dataTest: "source-filter"
      },
      slots: {
        default: '<button data-test="chip-a">A</button><button data-test="chip-b">B</button>',
        actions: '<button data-test="settings">Settings</button>'
      }
    });

    expect(wrapper.classes()).toContain("kx-chip-group");
    expect(wrapper.attributes("aria-label")).toBe("Sources");
    expect(wrapper.attributes("data-test")).toBe("source-filter");
    expect(wrapper.find(".kx-chip-group__chips [data-test='chip-a']").exists()).toBe(true);
    expect(wrapper.find(".kx-chip-group__actions [data-test='settings']").exists()).toBe(true);
  });
});
