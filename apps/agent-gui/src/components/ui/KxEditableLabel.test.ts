import { describe, expect, it, vi } from "vitest";
import { mount } from "@vue/test-utils";
import KxEditableLabel from "./KxEditableLabel.vue";

describe("KxEditableLabel", () => {
  it("renders a fixed inline edit input and emits confirm/cancel keyboard actions", async () => {
    const inputRef = vi.fn();
    const wrapper = mount(KxEditableLabel, {
      props: {
        modelValue: "Draft title",
        inputDataTest: "rename-input",
        confirmDataTest: "rename-confirm",
        confirmLabel: "Confirm"
      }
    });

    await wrapper.setProps({ inputRef });

    const input = wrapper.get<HTMLInputElement>('[data-test="rename-input"]');
    expect(input.classes()).toContain("kx-editable-label__input");
    expect(input.element.value).toBe("Draft title");
    expect(input.attributes("autocapitalize")).toBe("off");
    expect(input.attributes("autocomplete")).toBe("off");
    expect(input.attributes("autocorrect")).toBe("off");
    expect(input.attributes("spellcheck")).toBe("false");

    await input.setValue("Next title");
    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["Next title"]);

    await input.trigger("keydown.enter");
    await input.trigger("keydown.escape");

    expect(wrapper.emitted("confirm")).toHaveLength(1);
    expect(wrapper.emitted("cancel")).toHaveLength(1);
    expect(wrapper.get('[data-test="rename-confirm"]').classes()).toContain("kx-icon-button");
  });
});
