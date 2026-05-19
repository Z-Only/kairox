import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxInlineAlert from "./KxInlineAlert.vue";

describe("KxInlineAlert", () => {
  it("renders an error alert with semantic role and test selector", () => {
    const wrapper = mount(KxInlineAlert, {
      props: {
        tone: "error",
        dataTest: "validation-error"
      },
      slots: {
        default: "Missing command"
      }
    });

    expect(wrapper.attributes("role")).toBe("alert");
    expect(wrapper.attributes("data-test")).toBe("validation-error");
    expect(wrapper.classes()).toContain("kx-inline-alert--error");
    expect(wrapper.text()).toBe("Missing command");
  });

  it("uses status role for non-error feedback", () => {
    const wrapper = mount(KxInlineAlert, {
      props: { tone: "success" },
      slots: { default: "Installed" }
    });

    expect(wrapper.attributes("role")).toBe("status");
    expect(wrapper.classes()).toContain("kx-inline-alert--success");
  });
});
