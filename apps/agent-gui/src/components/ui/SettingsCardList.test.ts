import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SettingsCardList from "./SettingsCardList.vue";
import settingsCardListSource from "./SettingsCardList.vue?raw";

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

  it("supports an auto-column layout for lightweight settings rows", () => {
    const wrapper = mount(SettingsCardList, {
      props: {
        columns: "auto",
        dense: true,
        scroll: false,
        dataTest: "light-list"
      },
      slots: {
        default: "<article>One</article><article>Two</article>"
      }
    });

    expect(wrapper.attributes("data-test")).toBe("light-list");
    expect(wrapper.classes()).toContain("settings-card-list--auto-columns");
    expect(wrapper.classes()).toContain("settings-card-list--dense");
  });

  it("does not stretch a single card to fill available list height", () => {
    expect(settingsCardListSource).toContain("align-items: start");
  });
});
