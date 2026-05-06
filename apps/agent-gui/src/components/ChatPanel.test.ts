import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { mount } from "@vue/test-utils";
import { defineComponent, h } from "vue";
import {
  NConfigProvider,
  NMessageProvider,
  NDialogProvider,
  NLoadingBarProvider,
  NNotificationProvider
} from "naive-ui";
import ChatPanel from "./ChatPanel.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import { useSessionStore } from "@/stores/session";

// ChatPanel calls `useNotifications()` → `useMessage()` at setup; that hook
// throws unless wrapped under <NMessageProvider>. The composable's try/catch
// downgrades the failure to a console.error, but in tests we want a clean
// log, so we mount via this provider harness.
const ProviderHarness = defineComponent({
  name: "ProviderHarness",
  components: { ChatPanel },
  setup() {
    return () =>
      h(NConfigProvider, null, {
        default: () =>
          h(NLoadingBarProvider, null, {
            default: () =>
              h(NMessageProvider, null, {
                default: () =>
                  h(NDialogProvider, null, {
                    default: () =>
                      h(NNotificationProvider, null, {
                        default: () => h(ChatPanel)
                      })
                  })
              })
          })
      });
  }
});

function mountChatPanel() {
  return mount(ProviderHarness);
}

beforeEach(() => {
  setActivePinia(createPinia());
  const session = useSessionStore();
  session.resetProjection();
  session.currentSessionId = "ses_1";
  session.currentProfile = "fast";
  session.isStreaming = false;
  vi.clearAllMocks();
});

describe("ChatPanel", () => {
  it("renders user messages from projection", () => {
    const session = useSessionStore();
    session.projection.messages = [{ role: "user", content: "Hello" }];
    const wrapper = mountChatPanel();
    expect(wrapper.text()).toContain("Hello");
    expect(wrapper.text()).toContain("You");
  });

  it("renders assistant messages", () => {
    const session = useSessionStore();
    session.projection.messages = [{ role: "assistant", content: "Hi there!" }];
    const wrapper = mountChatPanel();
    expect(wrapper.text()).toContain("Hi there!");
    expect(wrapper.text()).toContain("Agent");
  });

  it("shows streaming text with cursor when isStreaming", () => {
    const session = useSessionStore();
    session.projection.token_stream = "Loading...";
    session.isStreaming = true;
    const wrapper = mountChatPanel();
    expect(wrapper.text()).toContain("Loading...");
    expect(wrapper.find(".cursor").exists()).toBe(true);
  });

  it("shows cancelled marker", () => {
    const session = useSessionStore();
    session.projection.cancelled = true;
    const wrapper = mountChatPanel();
    expect(wrapper.text()).toContain("[cancelled]");
    expect(wrapper.find('[data-test="cancelled-marker"]').exists()).toBe(true);
  });

  it("shows Cancel button during streaming", () => {
    const session = useSessionStore();
    session.isStreaming = true;
    const wrapper = mountChatPanel();
    expect(wrapper.find('[data-test="cancel-button"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="send-button"]').exists()).toBe(false);
  });

  it("shows Send button when not streaming", () => {
    const session = useSessionStore();
    session.isStreaming = false;
    const wrapper = mountChatPanel();
    expect(wrapper.find('[data-test="send-button"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="cancel-button"]').exists()).toBe(false);
  });

  it("disables the textarea when isStreaming", () => {
    const session = useSessionStore();
    session.isStreaming = true;
    const wrapper = mountChatPanel();
    // NInput renders a real <textarea> beneath; assert via the native element
    // because that's what the user actually interacts with.
    const textarea = wrapper.find('[data-test="message-input"] textarea');
    expect(textarea.exists()).toBe(true);
    expect(textarea.attributes("disabled")).toBeDefined();
  });

  it("invokes cancel_session on Cancel click", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const session = useSessionStore();
    session.isStreaming = true;
    const wrapper = mountChatPanel();
    await wrapper.find('[data-test="cancel-button"]').trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("cancel_session");
  });
});
