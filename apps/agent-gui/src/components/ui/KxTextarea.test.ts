import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxTextarea from "./KxTextarea.vue";

describe("KxTextarea", () => {
  it("renders a native textarea and forwards common attributes", () => {
    const wrapper = mount(KxTextarea, {
      props: {
        modelValue: "draft",
        dataTest: "shared-textarea",
        placeholder: "Write here",
        rows: 6,
        readonly: true,
        ariaLabel: "Shared textarea"
      }
    });

    const textarea = wrapper.get<HTMLTextAreaElement>("textarea");
    expect(textarea.classes()).toContain("kx-textarea");
    expect(textarea.attributes("data-test")).toBe("shared-textarea");
    expect(textarea.attributes("placeholder")).toBe("Write here");
    expect(textarea.attributes("rows")).toBe("6");
    expect(textarea.attributes("aria-label")).toBe("Shared textarea");
    expect(textarea.element.value).toBe("draft");
    expect(textarea.element.readOnly).toBe(true);
  });

  it("emits v-model updates and preserves native input events", async () => {
    const wrapper = mount(KxTextarea, {
      props: {
        modelValue: "",
        dataTest: "shared-textarea"
      }
    });

    await wrapper.get("textarea").setValue("updated");

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["updated"]);
    expect(wrapper.emitted("input")?.[0][0]).toBeInstanceOf(Event);
  });

  it("maps visual variants to stable classes", () => {
    const wrapper = mount(KxTextarea, {
      props: {
        modelValue: "preview",
        variant: "preview",
        resize: "none"
      }
    });

    const textarea = wrapper.get("textarea");
    expect(textarea.classes()).toContain("kx-textarea--preview");
    expect(textarea.classes()).toContain("kx-textarea--resize-none");
  });
});
