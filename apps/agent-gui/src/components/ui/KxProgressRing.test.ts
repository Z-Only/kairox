import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";

import KxProgressRing from "./KxProgressRing.vue";

describe("KxProgressRing", () => {
  it("renders an accessible SVG progress ring", () => {
    const wrapper = mount(KxProgressRing, {
      props: {
        ratio: 0.42,
        label: "42% context used",
        state: "warning"
      },
      slots: {
        default: "42%"
      }
    });

    const ring = wrapper.find('[data-test="progress-ring"]');

    expect(ring.exists()).toBe(true);
    expect(ring.attributes("aria-label")).toContain("context used");
    expect(ring.classes()).toContain("kx-progress-ring");
    expect(wrapper.find("svg").exists()).toBe(true);
    expect(wrapper.text()).toContain("42%");
  });

  it("falls back to zero when ratio is NaN", () => {
    const wrapper = mount(KxProgressRing, {
      props: {
        ratio: Number.NaN,
        label: "context used"
      }
    });

    const ring = wrapper.find('[data-test="progress-ring"]');

    expect(ring.attributes("aria-valuenow")).toBe("0");
    expect(ring.text()).toContain("0%");
    expect(ring.attributes("aria-valuenow")).not.toBe("NaN");
  });
});
