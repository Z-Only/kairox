import { describe, expect, it } from "vitest";
import { mount } from "@vue/test-utils";
import ScopeSelector from "./ScopeSelector.vue";
import scopeSelectorSource from "./ScopeSelector.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

function mountScopeSelector(props: Partial<InstanceType<typeof ScopeSelector>["$props"]> = {}) {
  return mount(ScopeSelector, {
    props: { modelValue: "User", ...props }
  });
}

describe("ScopeSelector", () => {
  it("uses shared KxRadioCard controls for scope choices", async () => {
    const wrapper = mountScopeSelector({ showLocal: true });

    expectSourceMigration(scopeSelectorSource, {
      required: ["KxRadioCard"],
      forbidden: [".scope-selector__option {"]
    });
    expect(wrapper.findAll(".kx-radio-card")).toHaveLength(3);

    await wrapper.find('[data-test="scope-project"] input').setValue(true);

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["Project"]);
  });

  it("hides the Local option by default", () => {
    const wrapper = mountScopeSelector();

    expect(wrapper.find('[data-test="scope-local"]').exists()).toBe(false);
    expect(wrapper.findAll(".kx-radio-card")).toHaveLength(2);
  });

  it("shows the Local option only when showLocal is true", async () => {
    const wrapper = mountScopeSelector({ showLocal: false });

    expect(wrapper.find('[data-test="scope-local"]').exists()).toBe(false);

    await wrapper.setProps({ showLocal: true });

    expect(wrapper.find('[data-test="scope-local"]').exists()).toBe(true);
    expect(wrapper.findAll(".kx-radio-card")).toHaveLength(3);
  });

  it("emits Local when selecting the Local option", async () => {
    const wrapper = mountScopeSelector({ showLocal: true });

    await wrapper.find('[data-test="scope-local"] input').setValue(true);

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["Local"]);
  });
});
