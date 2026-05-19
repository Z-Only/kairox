import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SettingsItemMeta from "./SettingsItemMeta.vue";

describe("SettingsItemMeta", () => {
  it("renders shared definition-list metadata with stable density classes", () => {
    const wrapper = mount(SettingsItemMeta, {
      props: {
        columns: "four",
        dataTest: "shared-meta"
      },
      slots: {
        default: "<div><dt>Path</dt><dd>/tmp/example</dd></div>"
      }
    });

    expect(wrapper.element.tagName).toBe("DL");
    expect(wrapper.attributes("data-test")).toBe("shared-meta");
    expect(wrapper.classes()).toContain("settings-item-meta");
    expect(wrapper.classes()).toContain("settings-item-meta--four");
    expect(wrapper.get("dt").text()).toBe("Path");
    expect(wrapper.get("dd").text()).toBe("/tmp/example");
  });

  it("supports compact wrapping metadata for lightweight two-column cards", () => {
    const wrapper = mount(SettingsItemMeta, {
      props: {
        compact: true,
        wrapValues: true
      },
      slots: {
        default: "<span>Project</span><span>Archived yesterday</span>"
      }
    });

    expect(wrapper.classes()).toContain("settings-item-meta--compact");
    expect(wrapper.classes()).toContain("settings-item-meta--wrap-values");
  });
});
