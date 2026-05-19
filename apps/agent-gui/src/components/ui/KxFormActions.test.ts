import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxFormActions from "./KxFormActions.vue";

describe("KxFormActions", () => {
  it("renders a shared action row for inline settings forms", () => {
    const wrapper = mount(KxFormActions, {
      props: {
        dataTest: "source-form-actions"
      },
      slots: {
        default: '<button class="btn btn-primary" data-test="save-source">Save</button>'
      }
    });

    expect(wrapper.classes()).toContain("kx-form-actions");
    expect(wrapper.attributes("data-test")).toBe("source-form-actions");
    expect(wrapper.find('[data-test="save-source"]').exists()).toBe(true);
  });

  it("can align dialog-adjacent actions to the end", () => {
    const wrapper = mount(KxFormActions, {
      props: {
        align: "end"
      },
      slots: {
        default: '<button class="btn">Cancel</button>'
      }
    });

    expect(wrapper.classes()).toContain("kx-form-actions--end");
  });
});
