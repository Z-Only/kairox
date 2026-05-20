import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SettingsCardItem from "./SettingsCardItem.vue";

describe("SettingsCardItem", () => {
  it("renders a split list item with shared card chrome", () => {
    const wrapper = mount(SettingsCardItem, {
      props: {
        dataTest: "settings-card-item"
      },
      slots: {
        default: "<span>Primary</span>"
      }
    });

    expect(wrapper.attributes("role")).toBe("listitem");
    expect(wrapper.attributes("data-test")).toBe("settings-card-item");
    expect(wrapper.classes()).toContain("settings-card-item");
    expect(wrapper.classes()).toContain("settings-card-item--split");
    expect(wrapper.text()).toContain("Primary");
  });

  it("supports stacked rows when a page needs vertical composition", () => {
    const wrapper = mount(SettingsCardItem, {
      props: {
        layout: "stack",
        density: "compact",
        role: "group"
      }
    });

    expect(wrapper.attributes("role")).toBe("group");
    expect(wrapper.classes()).toContain("settings-card-item--stack");
    expect(wrapper.classes()).toContain("settings-card-item--compact");
    expect(wrapper.classes()).not.toContain("settings-card-item--split");
  });

  it("places row actions in a shared action group without changing the card chrome", () => {
    const wrapper = mount(SettingsCardItem, {
      props: {
        actionsLabel: "Model profile actions"
      },
      slots: {
        default: "<span data-test='body'>Body</span>",
        actions: "<button data-test='edit'>Edit</button>",
        details: "<div data-test='details'>Details</div>"
      }
    });

    expect(wrapper.classes()).toContain("settings-card-item--with-actions");
    expect(wrapper.find(".settings-card-item__content [data-test='body']").exists()).toBe(true);
    const actions = wrapper.find(".settings-card-item__actions.kx-action-group");
    expect(actions.exists()).toBe(true);
    expect(actions.attributes("aria-label")).toBe("Model profile actions");
    expect(wrapper.find(".settings-card-item__actions [data-test='edit']").exists()).toBe(true);
    expect(wrapper.find(".settings-card-item__details [data-test='details']").exists()).toBe(true);
  });
});
