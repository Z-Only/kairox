import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import ModalDialog from "./ModalDialog.vue";

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = vi.fn();
  HTMLDialogElement.prototype.close = vi.fn();
});

describe("ModalDialog", () => {
  it("uses the shared Kx modal chrome and passes body selectors through", async () => {
    const wrapper = mount(ModalDialog, {
      props: { open: true, title: "Settings", bodyDataTest: "settings-body" },
      slots: { default: "<button>Inner action</button>" },
      attrs: { "data-test": "settings-dialog" }
    });

    expect(wrapper.find(".kx-modal__panel").exists()).toBe(true);
    expect(wrapper.get('[data-test="settings-dialog"]').exists()).toBe(true);
    expect(wrapper.get('[data-test="settings-body"]').text()).toContain("Inner action");
  });

  it("emits close when the backdrop is clicked", async () => {
    const wrapper = mount(ModalDialog, {
      props: { open: true, title: "Settings" },
      slots: { default: "<button>Inner action</button>" }
    });

    await wrapper.find("dialog").trigger("click", {
      clientX: 1,
      clientY: 1
    });

    expect(wrapper.emitted("close")).toHaveLength(1);
  });

  it("emits close from the shared close button", async () => {
    const wrapper = mount(ModalDialog, {
      props: { open: true, title: "Settings", closeLabel: "Close settings" },
      slots: { default: "<button>Inner action</button>" }
    });

    await wrapper.get(".kx-modal__close").trigger("click");

    expect(wrapper.emitted("close")).toHaveLength(1);
  });
});
