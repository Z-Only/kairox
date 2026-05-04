import { describe, it, expect } from "vitest";
import { mount } from "@vue/test-utils";
import ConfirmDialog from "./ConfirmDialog.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

describe("ConfirmDialog", () => {
  it("renders title and message", () => {
    const wrapper = mount(ConfirmDialog, {
      props: { title: "Delete Item?", message: "This cannot be undone." }
    });
    expect(wrapper.text()).toContain("Delete Item?");
    expect(wrapper.text()).toContain("This cannot be undone.");
  });

  it("emits confirm when confirm button is clicked", () => {
    const wrapper = mount(ConfirmDialog, {
      props: { title: "Confirm", message: "Are you sure?" }
    });
    wrapper.find(".btn-confirm").trigger("click");
    expect(wrapper.emitted("confirm")).toHaveLength(1);
  });

  it("emits cancel when cancel button is clicked", () => {
    const wrapper = mount(ConfirmDialog, {
      props: { title: "Confirm", message: "Are you sure?" }
    });
    wrapper.find(".btn-cancel").trigger("click");
    expect(wrapper.emitted("cancel")).toHaveLength(1);
  });

  it("applies danger style when confirmDanger is true", () => {
    const wrapper = mount(ConfirmDialog, {
      props: { title: "Delete?", message: "Permanent", confirmDanger: true }
    });
    expect(wrapper.find(".btn-confirm").classes()).toContain("btn-danger");
  });

  it("does not apply danger style by default", () => {
    const wrapper = mount(ConfirmDialog, {
      props: { title: "OK?", message: "Sure?" }
    });
    expect(wrapper.find(".btn-confirm").classes()).not.toContain("btn-danger");
  });
});
