import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxButton from "./KxButton.vue";
import kxButtonSource from "./KxButton.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

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

  it("keeps action variants in the owned component instead of global btn CSS", () => {
    expectSourceMigration(kxButtonSource, {
      required: [
        "type ButtonVariant",
        ".kx-button--danger",
        ".kx-button--danger-ghost",
        ".kx-button--ghost",
        ".kx-button--success",
        ".kx-button--warning"
      ],
      forbidden: [".btn-primary", ".btn-danger"]
    });
  });
});
