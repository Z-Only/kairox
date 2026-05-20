import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxFormField from "./KxFormField.vue";
import kxFormFieldSource from "./KxFormField.vue?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

describe("KxFormField", () => {
  it("renders a labelled control with optional description", () => {
    const wrapper = mount(KxFormField, {
      props: {
        label: "Source URL",
        description: "Use an HTTPS endpoint.",
        dataTest: "source-url-field"
      },
      slots: {
        default: '<input class="input" data-test="source-url-input" />'
      }
    });

    expect(wrapper.attributes("data-test")).toBe("source-url-field");
    expect(wrapper.find(".kx-form-field__label").text()).toBe("Source URL");
    expect(wrapper.find('[data-test="source-url-input"]').exists()).toBe(true);
    expect(wrapper.find(".kx-form-field__description").text()).toBe("Use an HTTPS endpoint.");
  });

  it("marks required fields without changing the accessible label text", () => {
    const wrapper = mount(KxFormField, {
      props: {
        label: "Search template",
        required: true
      },
      slots: {
        default: '<input class="input" />'
      }
    });

    expect(wrapper.find(".kx-form-field__label").text()).toBe("Search template *");
    expect(wrapper.find(".kx-form-field__required").attributes("aria-hidden")).toBe("true");
  });

  it("leaves control chrome to KxInput, KxSelect, and KxTextarea", () => {
    const wrapper = mount(KxFormField, {
      props: { label: "Provider" },
      slots: {
        default: '<input data-test="provider-input" />'
      }
    });

    expect(wrapper.find('[data-test="provider-input"]').exists()).toBe(true);
    expectSourceMigration(kxFormFieldSource, {
      forbidden: [
        ".kx-form-field :deep(.kx-form-control)",
        ".kx-form-field :deep(.kx-form-control:focus-visible)",
        ".kx-form-field :deep(.kx-form-control--textarea)",
        ".kx-form-field :deep(.kx-form-control--mono)"
      ]
    });
  });
});
