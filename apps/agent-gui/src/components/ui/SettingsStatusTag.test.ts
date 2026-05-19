import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SettingsStatusTag from "./SettingsStatusTag.vue";

describe("SettingsStatusTag", () => {
  it("renders neutral settings metadata tags by default", () => {
    const wrapper = mount(SettingsStatusTag, {
      slots: {
        default: "User"
      }
    });

    expect(wrapper.element.tagName).toBe("SPAN");
    expect(wrapper.classes()).toContain("tag");
    expect(wrapper.classes()).toContain("settings-status-tag");
    expect(wrapper.classes()).toContain("settings-status-tag--neutral");
    expect(wrapper.text()).toBe("User");
  });

  it("maps semantic tones to shared settings tag classes", () => {
    const wrapper = mount(SettingsStatusTag, {
      props: {
        tone: "success",
        dataTest: "enabled-tag"
      },
      slots: {
        default: "Enabled"
      }
    });

    expect(wrapper.attributes("data-test")).toBe("enabled-tag");
    expect(wrapper.classes()).toContain("settings-status-tag--success");
    expect(wrapper.classes()).not.toContain("tag-success");
  });

  it("supports source-specific tones without page-local source css", () => {
    const wrapper = mount(SettingsStatusTag, {
      props: {
        tone: "source-project"
      },
      slots: {
        default: "Project"
      }
    });

    expect(wrapper.classes()).toContain("settings-status-tag--source-project");
  });
});
