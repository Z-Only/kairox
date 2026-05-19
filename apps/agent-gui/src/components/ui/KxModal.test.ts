import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import KxModal from "./KxModal.vue";
import kxModalSource from "./KxModal.vue?raw";

beforeEach(() => {
  HTMLDialogElement.prototype.showModal = vi.fn();
  HTMLDialogElement.prototype.close = vi.fn();
});

describe("KxModal", () => {
  it("renders shared modal chrome with stable selectors", async () => {
    const wrapper = mount(KxModal, {
      props: {
        open: true,
        title: "Source settings",
        description: "Manage sources",
        bodyDataTest: "source-body",
        closeLabel: "Dismiss",
        width: "640px"
      },
      slots: {
        default: "<p>Catalog source content</p>",
        footer: "<button>Apply</button>"
      },
      attrs: {
        "data-test": "source-settings-dialog"
      }
    });

    await wrapper.vm.$nextTick();

    expect(wrapper.get('[data-test="source-settings-dialog"]').attributes("open")).toBeDefined();
    expect(wrapper.get(".kx-modal__panel").attributes("style")).toContain(
      "--kx-modal-width: 640px"
    );
    expect(wrapper.get(".kx-modal__title").text()).toBe("Source settings");
    expect(wrapper.get(".kx-modal__description").text()).toBe("Manage sources");
    expect(wrapper.get('[data-test="source-body"]').text()).toContain("Catalog source content");
    expect(wrapper.get(".kx-modal__footer").text()).toContain("Apply");
  });

  it("uses a full-viewport centering dialog so modal cards open in the page center", () => {
    expect(kxModalSource).toContain("place-items: center");
    expect(kxModalSource).toContain("width: 100vw");
    expect(kxModalSource).toContain("height: 100dvh");
    expect(kxModalSource).toContain("margin: 0");
  });

  it("emits close from close button and backdrop", async () => {
    const wrapper = mount(KxModal, {
      props: { open: true, title: "Install" }
    });

    await wrapper.get(".kx-modal__close").trigger("click");
    expect(wrapper.emitted("close")).toHaveLength(1);

    await wrapper.get("dialog").trigger("click");
    expect(wrapper.emitted("close")).toHaveLength(2);
  });
});
