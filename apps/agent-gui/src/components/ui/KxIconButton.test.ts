import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxIconButton from "./KxIconButton.vue";

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
});
