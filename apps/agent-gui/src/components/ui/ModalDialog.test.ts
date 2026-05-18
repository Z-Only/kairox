import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import ModalDialog from "./ModalDialog.vue";

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = vi.fn();
  HTMLDialogElement.prototype.close = vi.fn();
});

describe("ModalDialog", () => {
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
});
