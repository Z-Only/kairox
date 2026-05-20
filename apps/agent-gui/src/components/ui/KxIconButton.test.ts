import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxIconButton from "./KxIconButton.vue";
import kxIconButtonSource from "./KxIconButton.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

describe("KxIconButton", () => {
  it("renders an accessible native icon button", () => {
    const wrapper = mount(KxIconButton, {
      props: {
        label: "Rename",
        dataTest: "rename-button"
      },
      slots: {
        default: "✎"
      }
    });

    const button = wrapper.find('[data-test="rename-button"]');

    expect(button.element.tagName).toBe("BUTTON");
    expect(button.attributes("type")).toBe("button");
    expect(button.attributes("aria-label")).toBe("Rename");
    expect(button.attributes("title")).toBe("Rename");
    expect(button.classes()).toContain("kx-icon-button");
    expect(button.text()).toBe("✎");
  });

  it("owns icon button size and variant styling instead of relying on global btn classes", () => {
    const wrapper = mount(KxIconButton, {
      props: {
        label: "Close",
        variant: "default",
        size: "sm"
      },
      slots: {
        default: "x"
      }
    });

    const button = wrapper.get("button");
    expect(button.classes()).toContain("kx-icon-button--default");
    expect(button.classes()).toContain("kx-icon-button--size-sm");
    expectSourceMigration(kxIconButtonSource, {
      required: ["type IconButtonSize"],
      forbidden: ["btn-icon"]
    });
  });
});
