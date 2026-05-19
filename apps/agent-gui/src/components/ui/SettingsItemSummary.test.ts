import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SettingsItemSummary from "./SettingsItemSummary.vue";

describe("SettingsItemSummary", () => {
  it("renders a shared title, description, tags, and body layout", () => {
    const wrapper = mount(SettingsItemSummary, {
      props: {
        title: "GitHub",
        description: "Browse and manage repositories.",
        headingLevel: 4,
        tagsLabel: "Plugin metadata"
      },
      slots: {
        tags: '<span class="tag">User</span>',
        default: '<code data-test="path">/tmp/plugin</code>'
      }
    });

    expect(wrapper.classes()).toContain("settings-item-summary");
    expect(wrapper.get("h4.settings-item-summary__title").text()).toBe("GitHub");
    expect(wrapper.get(".settings-item-summary__description").text()).toBe(
      "Browse and manage repositories."
    );
    expect(wrapper.get(".settings-item-summary__tags").attributes("aria-label")).toBe(
      "Plugin metadata"
    );
    expect(wrapper.get('[data-test="path"]').text()).toBe("/tmp/plugin");
  });

  it("applies predictable line clamping for dense card descriptions", () => {
    const wrapper = mount(SettingsItemSummary, {
      props: {
        title: "Code Review",
        description: "Long description",
        descriptionLines: 1
      }
    });

    const description = wrapper.get(".settings-item-summary__description");
    expect(description.classes()).toContain("settings-item-summary__description--clamp");
    expect(description.attributes("style")).toContain("--settings-item-description-lines: 1");
  });
});
