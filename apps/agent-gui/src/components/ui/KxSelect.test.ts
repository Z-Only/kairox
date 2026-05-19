import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxSelect from "./KxSelect.vue";

describe("KxSelect", () => {
  it("renders a native select and forwards common attributes", () => {
    const wrapper = mount(KxSelect, {
      props: {
        modelValue: "user",
        dataTest: "shared-select",
        ariaLabel: "Shared select"
      },
      slots: {
        default: '<option value="user">User</option><option value="project">Project</option>'
      }
    });

    const select = wrapper.get<HTMLSelectElement>("select");
    expect(select.classes()).toContain("kx-select");
    expect(select.attributes("data-test")).toBe("shared-select");
    expect(select.attributes("aria-label")).toBe("Shared select");
    expect(select.element.value).toBe("user");
  });

  it("emits v-model updates and native change events", async () => {
    const wrapper = mount(KxSelect, {
      props: {
        modelValue: "user",
        dataTest: "shared-select"
      },
      slots: {
        default: '<option value="user">User</option><option value="project">Project</option>'
      }
    });

    await wrapper.get("select").setValue("project");

    expect(wrapper.emitted("update:modelValue")?.[0]).toEqual(["project"]);
    expect(wrapper.emitted("change")?.[0][0]).toBeInstanceOf(Event);
  });
});
