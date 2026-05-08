import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import ChatPanel from "./ChatPanel.vue";
import { mountWithPlugins } from "@/test-utils/mount";

// jsdom does not implement `Element.prototype.scrollTo`. The scrollbar
// calls it inside its own `scrollTo()` method when the message-list watcher
// fires (see ChatPanel.vue), which would surface as a noisy unhandled
// rejection during these tests even though no assertion depends on the
// scroll behaviour. Stub it once for the whole file.
if (typeof Element !== "undefined" && !Element.prototype.scrollTo) {
  Element.prototype.scrollTo = (() => {}) as Element["scrollTo"];
}

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import { useSessionStore } from "@/stores/session";

/**
 * `mountWithPlugins` activates a fresh Pinia internally, so the per-test
 * pattern is:
 *   1. mount the component (which sets the active Pinia)
 *   2. then read / mutate the session store via `useSessionStore()`
 * The `prepareSession` callback runs after mount and before assertions so
 * the Pinia instance the component sees is the same one the test mutates.
 */
function mountChatPanel(prepareSession?: (session: ReturnType<typeof useSessionStore>) => void) {
  const { wrapper } = mountWithPlugins(ChatPanel, {
    initialRoute: "/workbench"
  });
  const session = useSessionStore();
  session.resetProjection();
  session.currentSessionId = "ses_1";
  session.currentProfile = "fast";
  session.isStreaming = false;
  prepareSession?.(session);
  return wrapper;
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("ChatPanel", () => {
  it("renders user messages from projection", async () => {
    const wrapper = mountChatPanel((s) => {
      s.projection.messages = [{ role: "user", content: "Hello" }];
    });
    await flushPromises();
    expect(wrapper.text()).toContain("Hello");
    expect(wrapper.text()).toContain("You");
  });

  it("renders assistant messages", async () => {
    const wrapper = mountChatPanel((s) => {
      s.projection.messages = [{ role: "assistant", content: "Hi there!" }];
    });
    await flushPromises();
    expect(wrapper.text()).toContain("Hi there!");
    expect(wrapper.text()).toContain("Agent");
  });

  it("shows streaming text with cursor when isStreaming", async () => {
    const wrapper = mountChatPanel((s) => {
      s.projection.token_stream = "Loading...";
      s.isStreaming = true;
    });
    await flushPromises();
    expect(wrapper.text()).toContain("Loading...");
    expect(wrapper.find(".cursor").exists()).toBe(true);
  });

  it("shows cancelled marker", async () => {
    const wrapper = mountChatPanel((s) => {
      s.projection.cancelled = true;
    });
    await flushPromises();
    expect(wrapper.text()).toContain("[cancelled]");
    expect(wrapper.find('[data-test="cancelled-marker"]').exists()).toBe(true);
  });

  it("shows Cancel button during streaming", async () => {
    const wrapper = mountChatPanel((s) => {
      s.isStreaming = true;
    });
    await flushPromises();
    expect(wrapper.find('[data-test="cancel-button"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="send-button"]').exists()).toBe(false);
  });

  it("shows Send button when not streaming", async () => {
    const wrapper = mountChatPanel((s) => {
      s.isStreaming = false;
    });
    await flushPromises();
    expect(wrapper.find('[data-test="send-button"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="cancel-button"]').exists()).toBe(false);
  });

  it("disables the textarea when isStreaming", async () => {
    const wrapper = mountChatPanel((s) => {
      s.isStreaming = true;
    });
    await flushPromises();
    // Assert via the native <textarea> element because that's what the
    // user actually interacts with. The data-test attribute lives on the
    // <textarea> itself (not a wrapper), so we select it directly.
    const textarea = wrapper.find('textarea[data-test="message-input"]');
    expect(textarea.exists()).toBe(true);
    expect(textarea.attributes("disabled")).toBeDefined();
  });

  it("invokes cancel_session on Cancel click", async () => {
    mockedInvoke.mockResolvedValueOnce(undefined);
    const wrapper = mountChatPanel((s) => {
      s.isStreaming = true;
    });
    await flushPromises();
    await wrapper.find('[data-test="cancel-button"]').trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("cancel_session");
  });
});
