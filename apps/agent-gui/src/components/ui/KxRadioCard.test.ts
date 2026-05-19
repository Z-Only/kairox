import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import KxRadioCard from "./KxRadioCard.vue";

describe("KxRadioCard", () => {
  it("renders a selectable radio card and emits the selected value", async () => {
    const wrapper = mount(KxRadioCard, {
      props: {
        modelValue: "User",
        value: "Project",
        label: "Project",
        description: "Shared with the team",
        dataTest: "scope-project"
      }
    });

    expect(wrapper.classes()).toContain("kx-radio-card");
    expect(wrapper.find("input").attributes("type")).toBe("radio");
    expect(wrapper.find("input").element.checked).toBe(false);

    await wrapper.find("input").setValue(true);

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["Project"]);
  });
});
