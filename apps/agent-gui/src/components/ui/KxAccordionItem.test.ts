import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxAccordionItem from "./KxAccordionItem.vue";

describe("KxAccordionItem", () => {
  it("renders as a div by default", () => {
    const wrapper = mount(KxAccordionItem);

    expect(wrapper.element.tagName).toBe("DIV");
    expect(wrapper.classes()).toContain("kx-accordion-item");
    expect(wrapper.attributes("type")).toBeUndefined();
  });

  it("renders as a button with type='button'", () => {
    const wrapper = mount(KxAccordionItem, {
      props: { as: "button" }
    });

    expect(wrapper.element.tagName).toBe("BUTTON");
    expect(wrapper.attributes("type")).toBe("button");
    expect(wrapper.classes()).toContain("kx-accordion-item");
  });

  it("renders as an article element", () => {
    const wrapper = mount(KxAccordionItem, {
      props: { as: "article" }
    });

    expect(wrapper.element.tagName).toBe("ARTICLE");
    expect(wrapper.attributes("type")).toBeUndefined();
  });

  it("passes data-test attribute", () => {
    const wrapper = mount(KxAccordionItem, {
      props: { dataTest: "my-item" }
    });

    expect(wrapper.attributes("data-test")).toBe("my-item");
  });

  it("passes role attribute", () => {
    const wrapper = mount(KxAccordionItem, {
      props: { role: "listitem" }
    });

    expect(wrapper.attributes("role")).toBe("listitem");
  });

  it("omits role when not provided", () => {
    const wrapper = mount(KxAccordionItem);

    expect(wrapper.attributes("role")).toBeUndefined();
  });

  it("emits click event on click", async () => {
    const wrapper = mount(KxAccordionItem, {
      props: { as: "button" }
    });

    await wrapper.trigger("click");
    expect(wrapper.emitted("click")).toHaveLength(1);
    expect(wrapper.emitted("click")![0][0]).toBeInstanceOf(MouseEvent);
  });

  it("renders slot content", () => {
    const wrapper = mount(KxAccordionItem, {
      slots: { default: "<span>Hello</span>" }
    });

    expect(wrapper.find("span").text()).toBe("Hello");
  });

  it("passes through $attrs via inheritAttrs:false + v-bind", () => {
    const wrapper = mount(KxAccordionItem, {
      attrs: {
        "aria-expanded": "true",
        id: "item-1"
      }
    });

    expect(wrapper.attributes("aria-expanded")).toBe("true");
    expect(wrapper.attributes("id")).toBe("item-1");
  });
});
