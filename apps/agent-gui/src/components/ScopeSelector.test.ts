import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ScopeSelector from "./ScopeSelector.vue";
import scopeSelectorSource from "./ScopeSelector.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

describe("ScopeSelector", () => {
  it("uses shared KxRadioCard controls for scope choices", async () => {
    const wrapper = mount(ScopeSelector, {
      props: { modelValue: "User", showLocal: true }
    });

    expectSourceMigration(scopeSelectorSource, {
      required: ["KxRadioCard"],
      forbidden: [".scope-selector__option {"]
    });
    expect(wrapper.findAll(".kx-radio-card")).toHaveLength(3);

    await wrapper.find('[data-test="scope-project"] input').setValue(true);

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["Project"]);
  });
});
