import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxAccordionState from "./KxAccordionState.vue";

describe("KxAccordionState", () => {
  it("renders with the kx-accordion-state class", () => {
    const wrapper = mount(KxAccordionState);

    expect(wrapper.classes()).toContain("kx-accordion-state");
  });

  it("renders KxAsyncState with compact prop by default", () => {
    const wrapper = mount(KxAccordionState);
    const asyncState = wrapper.find(".kx-state-block");

    expect(asyncState.exists()).toBe(true);
    expect(asyncState.classes()).toContain("kx-state-block--compact");
  });

  it("defaults to empty tone", () => {
    const wrapper = mount(KxAccordionState);
    const asyncState = wrapper.find(".kx-state-block");

    expect(asyncState.classes()).toContain("kx-state-block--empty");
  });

  it.each(["empty", "loading", "info", "success", "warning", "error"] as const)(
    "renders with tone '%s'",
    (tone) => {
      const wrapper = mount(KxAccordionState, {
        props: { tone }
      });
      const asyncState = wrapper.find(".kx-state-block");

      expect(asyncState.classes()).toContain(`kx-state-block--${tone}`);
    }
  );

  it("passes data-test attribute to the outer wrapper", () => {
    const wrapper = mount(KxAccordionState, {
      props: { dataTest: "state-test" }
    });

    expect(wrapper.attributes("data-test")).toBe("state-test");
  });

  it("renders slot content through KxAsyncState", () => {
    const wrapper = mount(KxAccordionState, {
      props: { tone: "error" },
      slots: { default: "Something went wrong" }
    });

    expect(wrapper.text()).toContain("Something went wrong");
  });
});
