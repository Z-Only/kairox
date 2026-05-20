import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import SettingsFilterBar from "./SettingsFilterBar.vue";
import settingsFilterBarSource from "./SettingsFilterBar.vue?raw";
import SettingsSubtabs from "./SettingsSubtabs.vue";
import SettingsToolbar from "./SettingsToolbar.vue";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

describe("settings toolbar primitives", () => {
  it("renders a shared action toolbar with stable slots and attributes", () => {
    const wrapper = mount(SettingsToolbar, {
      props: {
        ariaLabel: "Model actions",
        dataTest: "model-toolbar"
      },
      slots: {
        default: "<button>Refresh</button>"
      }
    });

    expect(wrapper.classes()).toContain("settings-toolbar");
    expect(wrapper.attributes("role")).toBe("toolbar");
    expect(wrapper.attributes("aria-label")).toBe("Model actions");
    expect(wrapper.attributes("data-test")).toBe("model-toolbar");
    expect(wrapper.find("button").text()).toBe("Refresh");
  });

  it("does not emit optional labels or test hooks when omitted", () => {
    const wrapper = mount(SettingsToolbar);

    expect(wrapper.attributes("role")).toBe("toolbar");
    expect(wrapper.attributes("aria-label")).toBeUndefined();
    expect(wrapper.attributes("data-test")).toBeUndefined();
  });

  it("renders shared subtabs without changing tab button contracts", () => {
    const wrapper = mount(SettingsSubtabs, {
      props: {
        ariaLabel: "Sections",
        dataTest: "settings-subtabs"
      },
      slots: {
        default:
          '<button class="sub-tab-btn" role="tab" aria-selected="true" data-test="tab-a">A</button>'
      }
    });

    expect(wrapper.classes()).toContain("settings-subtabs");
    expect(wrapper.attributes("role")).toBe("tablist");
    expect(wrapper.attributes("aria-label")).toBe("Sections");
    expect(wrapper.attributes("data-test")).toBe("settings-subtabs");
    expect(wrapper.get('[data-test="tab-a"]').attributes("aria-selected")).toBe("true");
  });

  it("renders a filter bar for search and source controls", () => {
    const wrapper = mount(SettingsFilterBar, {
      props: {
        ariaLabel: "Filter plugins",
        dataTest: "catalog-filters"
      },
      slots: {
        default: '<input type="search" data-test="search" /><button>Search</button>'
      }
    });

    expect(wrapper.classes()).toContain("settings-filter-bar");
    expect(wrapper.attributes("aria-label")).toBe("Filter plugins");
    expect(wrapper.attributes("data-test")).toBe("catalog-filters");
    expect(wrapper.get('[data-test="search"]').attributes("type")).toBe("search");
    expectSourceMigration(settingsFilterBarSource, {
      forbidden: [".settings-filter-bar :deep(input)", ".settings-filter-bar :deep(select)"]
    });
  });
});
