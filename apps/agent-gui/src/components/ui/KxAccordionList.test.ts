import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxAccordionItem from "./KxAccordionItem.vue";
import KxAccordionList from "./KxAccordionList.vue";
import KxAccordionState from "./KxAccordionState.vue";

describe("KxAccordionList", () => {
  it("renders a compact list wrapper for nested accordion content", () => {
    const wrapper = mount(KxAccordionList, {
      props: {
        dataTest: "nested-list",
        ariaLabel: "Nested items"
      },
      slots: {
        default: "<span>Item</span>"
      }
    });

    expect(wrapper.classes()).toContain("kx-accordion-list");
    expect(wrapper.attributes("role")).toBeUndefined();
    expect(wrapper.attributes("aria-label")).toBe("Nested items");
    expect(wrapper.attributes("data-test")).toBe("nested-list");
  });

  it("renders rows as divs or buttons while keeping shared row chrome", async () => {
    const wrapper = mount(KxAccordionItem, {
      props: {
        as: "button",
        dataTest: "nested-row"
      },
      attrs: {
        "aria-expanded": "false"
      },
      slots: {
        default: "Open"
      }
    });

    expect(wrapper.element.tagName).toBe("BUTTON");
    expect(wrapper.attributes("type")).toBe("button");
    expect(wrapper.classes()).toContain("kx-accordion-item");
    expect(wrapper.attributes("role")).toBeUndefined();
    expect(wrapper.attributes("data-test")).toBe("nested-row");

    await wrapper.trigger("click");
    expect(wrapper.emitted("click")).toHaveLength(1);
  });

  it("wraps compact nested states without using page-level SettingsState", () => {
    const wrapper = mount(KxAccordionState, {
      props: {
        tone: "error",
        dataTest: "nested-error"
      },
      slots: {
        default: "Unable to load"
      }
    });

    expect(wrapper.classes()).toContain("kx-accordion-state");
    expect(wrapper.find(".kx-state-block").classes()).toContain("kx-state-block--compact");
    expect(wrapper.attributes("data-test")).toBe("nested-error");
    expect(wrapper.text()).toContain("Unable to load");
  });
});
