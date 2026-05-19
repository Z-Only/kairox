import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxInlineAction from "./KxInlineAction.vue";

describe("KxInlineAction", () => {
  it("renders compact row action buttons with shared KxButton chrome", () => {
    const wrapper = mount(KxInlineAction, {
      props: {
        variant: "danger",
        dataTest: "delete-row"
      },
      slots: {
        default: "Delete"
      }
    });

    const button = wrapper.get("button");
    expect(button.classes()).toEqual(
      expect.arrayContaining(["kx-button", "kx-button--danger", "kx-button--size-sm"])
    );
    expect(button.attributes("data-test")).toBe("delete-row");
    expect(button.text()).toBe("Delete");
  });
});
