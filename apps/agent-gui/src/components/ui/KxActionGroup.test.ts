import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxActionGroup from "./KxActionGroup.vue";

describe("KxActionGroup", () => {
  it("renders a shared wrapping action row with alignment variants", () => {
    const wrapper = mount(KxActionGroup, {
      props: {
        align: "end",
        ariaLabel: "Row actions",
        dataTest: "row-actions"
      },
      slots: {
        default: "<button>Edit</button><button>Delete</button>"
      }
    });

    expect(wrapper.attributes("aria-label")).toBe("Row actions");
    expect(wrapper.attributes("data-test")).toBe("row-actions");
    expect(wrapper.classes()).toContain("kx-action-group");
    expect(wrapper.classes()).toContain("kx-action-group--end");
    expect(wrapper.findAll("button")).toHaveLength(2);
  });
});
