import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxStateBlock from "./KxStateBlock.vue";

describe("KxStateBlock", () => {
  it("renders shared empty-state chrome with a stable test selector", () => {
    const wrapper = mount(KxStateBlock, {
      props: { tone: "empty", dataTest: "empty-state" },
      slots: { default: "Nothing configured" }
    });

    expect(wrapper.attributes("data-test")).toBe("empty-state");
    expect(wrapper.classes()).toContain("kx-state-block");
    expect(wrapper.classes()).toContain("kx-state-block--empty");
    expect(wrapper.text()).toBe("Nothing configured");
  });

  it("sets accessible default roles for loading and errors", () => {
    const loading = mount(KxStateBlock, {
      props: { tone: "loading" },
      slots: { default: "Loading" }
    });
    const error = mount(KxStateBlock, {
      props: { tone: "error" },
      slots: { default: "Failed" }
    });

    expect(loading.attributes("role")).toBe("status");
    expect(error.attributes("role")).toBe("alert");
  });

  it("allows callers to override the role when the state is contextual", () => {
    const wrapper = mount(KxStateBlock, {
      props: { tone: "info", role: "note" },
      slots: { default: "Contextual note" }
    });

    expect(wrapper.attributes("role")).toBe("note");
  });
});
