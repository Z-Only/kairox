import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxInput from "./KxInput.vue";

describe("KxInput", () => {
  it("renders a native input and forwards common attributes", () => {
    const wrapper = mount(KxInput, {
      props: {
        modelValue: "draft",
        dataTest: "shared-input",
        placeholder: "Search",
        type: "search",
        ariaLabel: "Shared input"
      }
    });

    const input = wrapper.get<HTMLInputElement>("input");
    expect(input.classes()).toContain("kx-input");
    expect(input.attributes("data-test")).toBe("shared-input");
    expect(input.attributes("placeholder")).toBe("Search");
    expect(input.attributes("type")).toBe("search");
    expect(input.attributes("aria-label")).toBe("Shared input");
    expect(input.element.value).toBe("draft");
  });

  it("emits v-model updates and native input events", async () => {
    const wrapper = mount(KxInput, {
      props: {
        modelValue: "",
        dataTest: "shared-input"
      }
    });

    await wrapper.get("input").setValue("updated");

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["updated"]);
    expect(wrapper.emitted("input")?.[0][0]).toBeInstanceOf(Event);
  });

  it("supports the v-model number modifier used by numeric forms", async () => {
    const wrapper = mount(KxInput, {
      props: {
        modelValue: 1,
        type: "number",
        modelModifiers: { number: true }
      }
    });

    await wrapper.get("input").setValue("42");

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual([42]);
  });
});
