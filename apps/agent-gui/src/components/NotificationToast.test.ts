import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount, flushPromises } from "@vue/test-utils";

// `useMessage()` is the only NaiveUI surface this component touches. We mock
// it with vi.fn-backed level handlers so the test can assert (a) each pushed
// notification is forwarded exactly once, (b) the store entry is dismissed
// afterwards so the visual lifecycle is owned by NaiveUI.
const messageStub = {
  info: vi.fn(),
  success: vi.fn(),
  warning: vi.fn(),
  error: vi.fn()
};

vi.mock("naive-ui", () => ({
  useMessage: () => messageStub
}));

import NotificationToast from "./NotificationToast.vue";
import { useUiStore } from "@/stores/ui";

beforeEach(() => {
  setActivePinia(createPinia());
  messageStub.info.mockReset();
  messageStub.success.mockReset();
  messageStub.warning.mockReset();
  messageStub.error.mockReset();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe("NotificationToast (useMessage adapter)", () => {
  it("forwards a pushed info notification to NaiveUI message.info() and clears the store entry", async () => {
    const ui = useUiStore();
    mount(NotificationToast);

    ui.pushNotification("info", "hello");
    await flushPromises();

    expect(messageStub.info).toHaveBeenCalledTimes(1);
    expect(messageStub.info).toHaveBeenCalledWith("hello");
    expect(ui.notifications).toEqual([]);
  });

  it("dispatches each level to its corresponding message.<level>() handler", async () => {
    const ui = useUiStore();
    mount(NotificationToast);

    ui.pushNotification("success", "ok");
    ui.pushNotification("warning", "careful");
    ui.pushNotification("error", "boom");
    await flushPromises();

    expect(messageStub.success).toHaveBeenCalledWith("ok");
    expect(messageStub.warning).toHaveBeenCalledWith("careful");
    // error level uses an extended duration so callers that catch a failure
    // get more time to read it; assert via the first positional arg.
    expect(messageStub.error).toHaveBeenCalledTimes(1);
    expect(messageStub.error.mock.calls[0][0]).toBe("boom");
    expect(ui.notifications).toEqual([]);
  });

  it("does not double-dispatch when the watcher fires again after dismiss", async () => {
    const ui = useUiStore();
    mount(NotificationToast);

    ui.pushNotification("info", "once");
    await flushPromises();
    // Push another after the first is drained — must dispatch independently.
    ui.pushNotification("info", "twice");
    await flushPromises();

    expect(messageStub.info).toHaveBeenCalledTimes(2);
    expect(messageStub.info).toHaveBeenNthCalledWith(1, "once");
    expect(messageStub.info).toHaveBeenNthCalledWith(2, "twice");
    expect(ui.notifications).toEqual([]);
  });

  it("renders no markup of its own (visual layer is owned by NMessageProvider)", () => {
    const wrapper = mount(NotificationToast);
    // Comment-only template — wrapper.text() should be empty and there
    // should be no host elements at all.
    expect(wrapper.text()).toBe("");
    expect(wrapper.find(".notification-container").exists()).toBe(false);
  });
});
