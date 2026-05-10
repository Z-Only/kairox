import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxPopover from "./KxPopover.vue";

describe("KxPopover", () => {
  it("renders trigger and open popover content", () => {
    const hostElement = document.createElement("div");
    document.body.appendChild(hostElement);

    const wrapper = mount(KxPopover, {
      attachTo: hostElement,
      props: {
        open: true,
        contentDataTest: "popover-content"
      },
      slots: {
        trigger: '<button data-test="popover-trigger" type="button">Details</button>',
        default: "<section>Context details</section>"
      },
      global: {
        stubs: {
          Teleport: true
        }
      }
    });

    try {
      expect(wrapper.find('[data-test="popover-trigger"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="popover-content"]').exists()).toBe(true);
      expect(wrapper.find('[data-test="popover-content"]').classes()).toContain(
        "kx-popover-content"
      );
      expect(wrapper.find('[data-test="popover-content"]').text()).toContain("Context details");
    } finally {
      wrapper.unmount();
      hostElement.remove();
    }
  });
});
