import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxButton from "./KxButton.vue";
import kxButtonSource from "./KxButton.vue?raw";

describe("KxButton", () => {
  it("renders shared text button chrome with variant and size classes", () => {
    const wrapper = mount(KxButton, {
      props: {
        variant: "primary",
        size: "sm",
        dataTest: "save-button"
      },
      slots: {
        default: "Save"
      }
    });

    const button = wrapper.get("button");
    expect(button.classes()).toContain("kx-button");
    expect(button.classes()).toContain("kx-button--primary");
    expect(button.classes()).toContain("kx-button--size-sm");
    expect(button.attributes("data-test")).toBe("save-button");
    expect(button.attributes("type")).toBe("button");
    expect(button.text()).toBe("Save");
  });

  it("keeps danger and ghost variants in the owned component instead of global btn CSS", () => {
    expect(kxButtonSource).toContain("type ButtonVariant");
    expect(kxButtonSource).toContain(".kx-button--danger");
    expect(kxButtonSource).toContain(".kx-button--ghost");
    expect(kxButtonSource).not.toContain(".btn-primary");
    expect(kxButtonSource).not.toContain(".btn-danger");
  });
});
