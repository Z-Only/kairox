import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import { useUiStore } from "@/stores/ui";
import ToastContainer from "./ToastContainer.vue";

function mountToastContainer() {
  return mountWithPlugins(ToastContainer, {
    reusePinia: true,
    mount: {
      global: {
        stubs: { Teleport: true }
      }
    }
  }).wrapper;
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe("ToastContainer", () => {
  describe("empty state", () => {
    it("renders the container with no toasts when store is empty", () => {
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast-container").exists()).toBe(true);
      expect(wrapper.findAll(".toast")).toHaveLength(0);
    });

    it("sets aria-live=polite on the container for accessibility", () => {
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast-container").attributes("aria-live")).toBe("polite");
    });
  });

  describe("rendering toasts", () => {
    it("renders a toast for each item in the store", () => {
      const ui = useUiStore();
      ui.addToast("First message", "info");
      ui.addToast("Second message", "success");
      const wrapper = mountToastContainer();
      expect(wrapper.findAll(".toast")).toHaveLength(2);
    });

    it("displays the toast message text", () => {
      const ui = useUiStore();
      ui.addToast("Hello world", "info");
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast__message").text()).toBe("Hello world");
    });

    it("applies the correct type class for success toasts", () => {
      const ui = useUiStore();
      ui.addToast("Done", "success");
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast--success").exists()).toBe(true);
    });

    it("applies the correct type class for error toasts", () => {
      const ui = useUiStore();
      ui.addToast("Fail", "error");
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast--error").exists()).toBe(true);
    });

    it("applies the correct type class for warning toasts", () => {
      const ui = useUiStore();
      ui.addToast("Caution", "warning");
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast--warning").exists()).toBe(true);
    });

    it("applies the correct type class for info toasts", () => {
      const ui = useUiStore();
      ui.addToast("Note", "info");
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast--info").exists()).toBe(true);
    });

    it("sets role=alert on each toast", () => {
      const ui = useUiStore();
      ui.addToast("Alert", "error");
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast").attributes("role")).toBe("alert");
    });
  });

  describe("toast icons", () => {
    it("shows check icon for success toasts", () => {
      const ui = useUiStore();
      ui.addToast("ok", "success");
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast__icon").text()).toBe("✓");
    });

    it("shows cross icon for error toasts", () => {
      const ui = useUiStore();
      ui.addToast("err", "error");
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast__icon").text()).toBe("✕");
    });

    it("shows warning icon for warning toasts", () => {
      const ui = useUiStore();
      ui.addToast("warn", "warning");
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast__icon").text()).toBe("⚠");
    });

    it("shows info icon for info toasts", () => {
      const ui = useUiStore();
      ui.addToast("info", "info");
      const wrapper = mountToastContainer();
      expect(wrapper.find(".toast__icon").text()).toBe("ℹ");
    });
  });

  describe("dismiss button", () => {
    it("renders a close button for each toast", () => {
      const ui = useUiStore();
      ui.addToast("Dismissable", "info");
      const wrapper = mountToastContainer();
      const closeBtn = wrapper.find(".toast__close");
      expect(closeBtn.exists()).toBe(true);
      expect(closeBtn.attributes("aria-label")).toBe("Dismiss");
    });

    it("removes the toast from the store when dismiss is clicked", async () => {
      const ui = useUiStore();
      ui.addToast("Remove me", "info");
      const wrapper = mountToastContainer();
      expect(wrapper.findAll(".toast")).toHaveLength(1);

      await wrapper.find(".toast__close").trigger("click");
      await flushPromises();
      expect(ui.toasts).toHaveLength(0);
      expect(wrapper.findAll(".toast")).toHaveLength(0);
    });

    it("removes only the clicked toast when multiple are present", async () => {
      const ui = useUiStore();
      ui.addToast("Keep me", "info");
      ui.addToast("Remove me", "error");
      const wrapper = mountToastContainer();
      expect(wrapper.findAll(".toast")).toHaveLength(2);

      // Click the close button on the second toast
      const closeBtns = wrapper.findAll(".toast__close");
      await closeBtns[1].trigger("click");
      await flushPromises();

      expect(ui.toasts).toHaveLength(1);
      expect(wrapper.findAll(".toast")).toHaveLength(1);
      expect(wrapper.find(".toast__message").text()).toBe("Keep me");
    });
  });

  describe("auto-dismiss via duration", () => {
    it("removes toast after its duration elapses", async () => {
      const ui = useUiStore();
      ui.addToast("Timed", "info", 3000);
      const wrapper = mountToastContainer();
      // Force the @vue:mounted handler to run
      await flushPromises();

      expect(wrapper.findAll(".toast")).toHaveLength(1);

      vi.advanceTimersByTime(3000);
      await flushPromises();

      expect(ui.toasts).toHaveLength(0);
    });

    it("does not auto-dismiss when duration is 0", async () => {
      const ui = useUiStore();
      ui.addToast("Sticky", "info", 0);
      const wrapper = mountToastContainer();
      await flushPromises();

      vi.advanceTimersByTime(10000);
      await flushPromises();

      expect(ui.toasts).toHaveLength(1);
      expect(wrapper.findAll(".toast")).toHaveLength(1);
    });
  });

  describe("multiple toasts", () => {
    it("renders all four toast types simultaneously", () => {
      const ui = useUiStore();
      ui.addToast("Info", "info");
      ui.addToast("Success", "success");
      ui.addToast("Warning", "warning");
      ui.addToast("Error", "error");
      const wrapper = mountToastContainer();

      expect(wrapper.findAll(".toast")).toHaveLength(4);
      expect(wrapper.find(".toast--info").exists()).toBe(true);
      expect(wrapper.find(".toast--success").exists()).toBe(true);
      expect(wrapper.find(".toast--warning").exists()).toBe(true);
      expect(wrapper.find(".toast--error").exists()).toBe(true);
    });

    it("reflects store additions reactively", async () => {
      const ui = useUiStore();
      const wrapper = mountToastContainer();
      expect(wrapper.findAll(".toast")).toHaveLength(0);

      ui.addToast("Added later", "success");
      await flushPromises();
      expect(wrapper.findAll(".toast")).toHaveLength(1);
    });
  });

  describe("teleport", () => {
    it("renders inside a Teleport targeting body (stubbed)", () => {
      const wrapper = mountToastContainer();
      // With Teleport stubbed, content renders inline
      expect(wrapper.find(".toast-container").exists()).toBe(true);
    });
  });
});
