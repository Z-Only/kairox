import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxEmptyState from "./KxEmptyState.vue";

describe("KxEmptyState", () => {
  it("wraps KxAsyncState with empty tone and stable selector support", () => {
    const wrapper = mount(KxEmptyState, {
      props: { dataTest: "empty-state", description: "Nothing to show" },
      slots: {
        default: "No items",
        actions: "<button>Create</button>"
      }
    });

    expect(wrapper.attributes("data-test")).toBe("empty-state");
    expect(wrapper.classes()).toContain("kx-empty-state");
    expect(wrapper.classes()).toContain("kx-async-state");
    expect(wrapper.classes()).toContain("kx-async-state--empty");
    expect(wrapper.classes()).toContain("kx-state-block--empty");
    expect(wrapper.find(".kx-async-state__message").text()).toBe("No items");
    expect(wrapper.find(".kx-async-state__description").text()).toBe("Nothing to show");
    expect(wrapper.find(".kx-async-state__actions button").text()).toBe("Create");
  });

  it("exposes density classes for compact inline and popover empty states", () => {
    const inline = mount(KxEmptyState, {
      props: { density: "inline" },
      slots: { default: "No rows" }
    });
    const popover = mount(KxEmptyState, {
      props: { density: "popover", dataTest: "popover-empty" },
      slots: { default: "No matches" }
    });

    expect(inline.classes()).toContain("kx-empty-state--inline");
    expect(inline.classes()).toContain("kx-state-block--compact");
    expect(popover.classes()).toContain("kx-empty-state--popover");
    expect(popover.classes()).toContain("kx-popover-empty");
    expect(popover.attributes("data-test")).toBe("popover-empty");
  });
});
