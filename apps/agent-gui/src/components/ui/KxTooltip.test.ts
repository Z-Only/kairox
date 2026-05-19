import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxTooltip from "./KxTooltip.vue";

describe("KxTooltip", () => {
  it("renders trigger and open tooltip content with stable anchors", () => {
    const hostElement = document.createElement("div");
    document.body.appendChild(hostElement);

    const wrapper = mount(KxTooltip, {
      attachTo: hostElement,
      props: {
        open: true,
        text: "Create session",
        contentDataTest: "tooltip-content",
        side: "right",
        sideOffset: 10
      },
      slots: {
        default: '<button data-test="tooltip-trigger" type="button">New</button>'
      },
      global: {
        stubs: {
          Teleport: true
        }
      }
    });

    try {
      expect(wrapper.find('[data-test="tooltip-trigger"]').exists()).toBe(true);
      const content = wrapper.find('[data-test="tooltip-content"]');
      expect(content.exists()).toBe(true);
      expect(content.classes()).toContain("kx-tooltip-content");
      expect(content.text()).toBe("Create session");
    } finally {
      wrapper.unmount();
      hostElement.remove();
    }
  });
});
