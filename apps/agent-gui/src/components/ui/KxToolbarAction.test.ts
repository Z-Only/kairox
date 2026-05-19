import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxToolbarAction from "./KxToolbarAction.vue";

describe("KxToolbarAction", () => {
  it("renders the standard compact toolbar button chrome", () => {
    const wrapper = mount(KxToolbarAction, {
      props: {
        variant: "primary",
        dataTest: "add-item",
        title: "Add item"
      },
      slots: {
        default: "Add"
      }
    });

    const button = wrapper.get("button");
    expect(button.classes()).toEqual(
      expect.arrayContaining(["kx-button", "kx-button--primary", "kx-button--size-sm"])
    );
    expect(button.attributes("data-test")).toBe("add-item");
    expect(button.attributes("title")).toBe("Add item");
    expect(button.text()).toBe("Add");
  });
});
