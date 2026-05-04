import { describe, it, expect, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import NotificationToast from "./NotificationToast.vue";
import { notifications } from "../composables/useNotifications";

beforeEach(() => {
  notifications.splice(0, notifications.length);
});

describe("NotificationToast", () => {
  it("does not render container when no notifications", () => {
    const wrapper = mount(NotificationToast);
    expect(wrapper.find(".notification-container").exists()).toBe(false);
  });

  it("renders up to 3 notifications", () => {
    notifications.push(
      { id: "1", type: "error", message: "Error 1", timestamp: Date.now() },
      { id: "2", type: "warning", message: "Warning 2", timestamp: Date.now() },
      { id: "3", type: "info", message: "Info 3", timestamp: Date.now() },
      { id: "4", type: "error", message: "Error 4", timestamp: Date.now() }
    );
    const wrapper = mount(NotificationToast);
    const items = wrapper.findAll(".notification");
    expect(items).toHaveLength(3);
  });

  it("applies CSS class based on notification type", () => {
    notifications.push(
      { id: "1", type: "error", message: "Oops", timestamp: Date.now() },
      { id: "2", type: "warning", message: "Careful", timestamp: Date.now() },
      { id: "3", type: "info", message: "FYI", timestamp: Date.now() }
    );
    const wrapper = mount(NotificationToast);
    const items = wrapper.findAll(".notification");
    expect(items[0].classes()).toContain("notification--error");
    expect(items[1].classes()).toContain("notification--warning");
    expect(items[2].classes()).toContain("notification--info");
  });

  it("calls dismissNotification when dismiss button is clicked", () => {
    notifications.push({
      id: "1",
      type: "error",
      message: "Dismiss me",
      timestamp: Date.now()
    });
    const wrapper = mount(NotificationToast);
    wrapper.find(".notification-dismiss").trigger("click");
    expect(notifications).toHaveLength(0);
  });

  it("shows correct icon for each type", () => {
    notifications.push(
      { id: "1", type: "error", message: "E", timestamp: Date.now() },
      { id: "2", type: "warning", message: "W", timestamp: Date.now() },
      { id: "3", type: "info", message: "I", timestamp: Date.now() }
    );
    const wrapper = mount(NotificationToast);
    const icons = wrapper.findAll(".notification-icon");
    expect(icons[0].text()).toBe("✕");
    expect(icons[1].text()).toBe("⚠");
    expect(icons[2].text()).toBe("ℹ");
  });
});
