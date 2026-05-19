import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SettingsCardList from "./SettingsCardList.vue";

describe("SettingsCardList", () => {
  it("renders shared list chrome with stable accessibility attributes", () => {
    const wrapper = mount(SettingsCardList, {
      props: {
        ariaLabel: "Configured items",
        dataTest: "settings-card-list"
      },
      slots: {
        default: "<article>Item</article>"
      }
    });

    expect(wrapper.attributes("role")).toBe("list");
    expect(wrapper.attributes("aria-label")).toBe("Configured items");
    expect(wrapper.attributes("data-test")).toBe("settings-card-list");
    expect(wrapper.classes()).toContain("settings-card-list");
    expect(wrapper.classes()).toContain("settings-card-list--scroll");
    expect(wrapper.text()).toContain("Item");
  });

  it("can render as a dense non-scrolling list for nested panels", () => {
    const wrapper = mount(SettingsCardList, {
      props: {
        scroll: false,
        dense: true
      }
    });

    expect(wrapper.classes()).not.toContain("settings-card-list--scroll");
    expect(wrapper.classes()).toContain("settings-card-list--dense");
  });
});
