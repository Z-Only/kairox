import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxBadge from "./KxBadge.vue";

describe("KxBadge", () => {
  it("renders status badges on top of KxTag styling", () => {
    const wrapper = mount(KxBadge, {
      props: {
        tone: "warning",
        dataTest: "retry-badge"
      },
      slots: {
        default: "pending"
      }
    });

    expect(wrapper.classes()).toContain("tag");
    expect(wrapper.classes()).toContain("kx-tag");
    expect(wrapper.classes()).toContain("kx-badge");
    expect(wrapper.classes()).toContain("kx-tag--warning");
    expect(wrapper.classes()).not.toContain("tag-warning");
    expect(wrapper.attributes("data-test")).toBe("retry-badge");
  });
});
