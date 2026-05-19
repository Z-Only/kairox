import { mount } from "@vue/test-utils";
import { describe, expect, it } from "vitest";
import KxAsyncState from "./KxAsyncState.vue";

describe("KxAsyncState", () => {
  it("renders loading and error states with accessible default roles", () => {
    const loading = mount(KxAsyncState, {
      props: { tone: "loading", dataTest: "loading-state" },
      slots: { default: "Loading data" }
    });
    const error = mount(KxAsyncState, {
      props: { tone: "error", dataTest: "error-state" },
      slots: { default: "Failed to load data" }
    });

    expect(loading.attributes("data-test")).toBe("loading-state");
    expect(loading.attributes("role")).toBe("status");
    expect(loading.classes()).toContain("kx-async-state");
    expect(loading.classes()).toContain("kx-async-state--loading");
    expect(loading.find(".kx-async-state__message").text()).toBe("Loading data");

    expect(error.attributes("data-test")).toBe("error-state");
    expect(error.attributes("role")).toBe("alert");
    expect(error.classes()).toContain("kx-async-state--error");
  });

  it("supports title, description, and action slots without changing the outer state chrome", () => {
    const wrapper = mount(KxAsyncState, {
      props: {
        tone: "empty",
        title: "No results",
        description: "Try a different filter.",
        dataTest: "empty-state"
      },
      slots: {
        actions: "<button>Retry</button>"
      }
    });

    expect(wrapper.classes()).toContain("kx-state-block");
    expect(wrapper.classes()).toContain("kx-state-block--empty");
    expect(wrapper.find(".kx-async-state__title").text()).toBe("No results");
    expect(wrapper.find(".kx-async-state__description").text()).toBe("Try a different filter.");
    expect(wrapper.find(".kx-async-state__actions button").text()).toBe("Retry");
  });

  it("allows contextual role overrides", () => {
    const wrapper = mount(KxAsyncState, {
      props: { tone: "info", role: "note" },
      slots: { default: "Contextual state" }
    });

    expect(wrapper.attributes("role")).toBe("note");
  });
});
