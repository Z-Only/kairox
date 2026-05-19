import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxChipButton from "./KxChipButton.vue";

describe("KxChipButton", () => {
  it("renders a compact selectable filter chip with stable state attributes", () => {
    const wrapper = mount(KxChipButton, {
      props: {
        selected: true,
        size: "compact",
        dataTest: "source-chip"
      },
      slots: {
        default: "Built-in"
      }
    });

    const button = wrapper.get("button");
    expect(button.classes()).toEqual(
      expect.arrayContaining([
        "kx-chip-button",
        "kx-chip-button--selected",
        "kx-chip-button--size-compact"
      ])
    );
    expect(button.attributes("aria-pressed")).toBe("true");
    expect(button.attributes("data-test")).toBe("source-chip");
    expect(button.text()).toBe("Built-in");
  });

  it("supports disabled chips without custom page CSS", () => {
    const wrapper = mount(KxChipButton, {
      props: {
        disabled: true
      },
      slots: {
        default: "Remote"
      }
    });

    const button = wrapper.get("button");
    expect(button.attributes("disabled")).toBeDefined();
    expect(button.classes()).toContain("kx-chip-button--default");
  });
});
