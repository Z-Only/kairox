import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxTag from "./KxTag.vue";

describe("KxTag", () => {
  it("renders a neutral inline tag by default", () => {
    const wrapper = mount(KxTag, {
      slots: {
        default: "workspace"
      }
    });

    expect(wrapper.element.tagName).toBe("SPAN");
    expect(wrapper.classes()).toContain("tag");
    expect(wrapper.classes()).toContain("kx-tag");
    expect(wrapper.classes()).toContain("kx-tag--neutral");
    expect(wrapper.text()).toBe("workspace");
  });

  it("uses semantic tone classes instead of legacy tag classes", () => {
    const wrapper = mount(KxTag, {
      props: {
        tone: "success",
        size: "sm",
        dataTest: "status-tag"
      },
      slots: {
        default: "running"
      }
    });

    expect(wrapper.attributes("data-test")).toBe("status-tag");
    expect(wrapper.classes()).toContain("kx-tag--success");
    expect(wrapper.classes()).toContain("kx-tag--sm");
    expect(wrapper.classes()).not.toContain("tag-success");
  });

  it("can render links while preserving tag styling", () => {
    const wrapper = mount(KxTag, {
      props: {
        as: "a",
        tone: "info"
      },
      attrs: {
        href: "https://example.test",
        target: "_blank",
        rel: "noopener noreferrer"
      },
      slots: {
        default: "Source"
      }
    });

    expect(wrapper.element.tagName).toBe("A");
    expect(wrapper.attributes("href")).toBe("https://example.test");
    expect(wrapper.classes()).toContain("kx-tag--info");
  });
});
