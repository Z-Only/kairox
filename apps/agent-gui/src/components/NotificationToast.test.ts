import { describe, it, expect, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import NotificationToast from "./NotificationToast.vue";
import { useUiStore } from "@/stores/ui";

beforeEach(() => {
  setActivePinia(createPinia());
});

describe("NotificationToast", () => {
  it("does not render container when no notifications", () => {
    const wrapper = mount(NotificationToast);
    expect(wrapper.find(".notification-container").exists()).toBe(false);
  });

  it("renders up to 3 notifications", () => {
    const ui = useUiStore();
    ui.pushNotification("error", "Error 1");
    ui.pushNotification("warning", "Warning 2");
    ui.pushNotification("info", "Info 3");
    ui.pushNotification("error", "Error 4");
    const wrapper = mount(NotificationToast);
    const items = wrapper.findAll(".notification");
    expect(items).toHaveLength(3);
  });

  it("applies CSS class based on notification level", () => {
    const ui = useUiStore();
    ui.pushNotification("error", "Oops");
    ui.pushNotification("warning", "Careful");
    ui.pushNotification("info", "FYI");
    const wrapper = mount(NotificationToast);
    const items = wrapper.findAll(".notification");
    expect(items[0].classes()).toContain("notification--error");
    expect(items[1].classes()).toContain("notification--warning");
    expect(items[2].classes()).toContain("notification--info");
  });

  it("calls dismissNotification when dismiss button is clicked", async () => {
    const ui = useUiStore();
    ui.pushNotification("error", "Dismiss me");
    const wrapper = mount(NotificationToast);
    await wrapper.find(".notification-dismiss").trigger("click");
    expect(ui.notifications).toHaveLength(0);
  });

  it("shows correct icon for each level", () => {
    const ui = useUiStore();
    ui.pushNotification("error", "E");
    ui.pushNotification("warning", "W");
    ui.pushNotification("info", "I");
    const wrapper = mount(NotificationToast);
    const icons = wrapper.findAll(".notification-icon");
    expect(icons[0].text()).toBe("✕");
    expect(icons[1].text()).toBe("⚠");
    expect(icons[2].text()).toBe("ℹ");
  });
});
