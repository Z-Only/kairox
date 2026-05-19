import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import KxActionButton from "./KxActionButton.vue";

describe("KxActionButton", () => {
  it("renders compact shared action-button chrome with variant classes", () => {
    const wrapper = mount(KxActionButton, {
      props: { variant: "danger", dataTest: "delete-action" },
      slots: { default: "Delete" }
    });

    const button = wrapper.get("button");
    expect(button.classes()).toContain("kx-action-button");
    expect(button.classes()).toContain("kx-action-button--danger");
    expect(button.attributes("data-test")).toBe("delete-action");
    expect(button.text()).toBe("Delete");
  });
});
