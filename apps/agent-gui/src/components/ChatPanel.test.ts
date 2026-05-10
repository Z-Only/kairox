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
import type { ContextUsage } from "@/types";

/**
 * `mountWithPlugins` activates a fresh Pinia internally, so the per-test
 * pattern is:
 *   1. mount the component (which sets the active Pinia)
 *   2. then read / mutate the session store via `useSessionStore()`
 * The `prepareSession` callback runs after mount and before assertions so
 * the Pinia instance the component sees is the same one the test mutates.
 */
function makeUsage(overrides: Partial<ContextUsage> = {}): ContextUsage {
  return {
    total_tokens: 50,
    budget_tokens: 100,
    context_window: 120,
    output_reservation: 20,
    by_source: [["history", 50]],
    estimator: "cl100k_base",
    corrected_by_real_usage: false,
    ...overrides
  };
}

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
  it("renders message bubbles without visible role labels while preserving chat-message anchors", async () => {
    const wrapper = mountChatPanel((session) => {
      session.projection.messages = [
        { role: "user", content: "Hello" },
        { role: "assistant", content: "Hi there!" }
      ];
    });
    await flushPromises();

    expect(wrapper.findAll('[data-test="chat-message"]')).toHaveLength(2);
    expect(wrapper.find('[data-test="chat-message"][data-role="user"]').text()).toBe("Hello");
    expect(wrapper.find('[data-test="chat-message"][data-role="assistant"]').text()).toBe(
      "Hi there!"
    );
    expect(wrapper.find(".message-role").exists()).toBe(false);
    expect(wrapper.text()).not.toContain("You");
    expect(wrapper.text()).not.toContain("Agent");
  });

  it("renders streaming text and cursor without visible assistant role labels", async () => {
    const wrapper = mountChatPanel((session) => {
      session.projection.token_stream = "Loading...";
      session.isStreaming = true;
    });
    await flushPromises();

    const streamIndicator = wrapper.find('[data-test="stream-indicator"]');
    expect(streamIndicator.exists()).toBe(true);
    expect(streamIndicator.text()).toContain("Loading...");
    expect(streamIndicator.find(".cursor").exists()).toBe(true);
    expect(streamIndicator.find(".message-role").exists()).toBe(false);
    expect(streamIndicator.text()).not.toContain("Agent");
  });

  it("shows the current profile badge in the composer input area", async () => {
    const wrapper = mountChatPanel();
    await flushPromises();

    const header = wrapper.find(".chat-header");
    const inputArea = wrapper.find(".input-area");
    const profileBadge = inputArea.find('[data-test="chat-profile-badge"]');

    expect(profileBadge.exists()).toBe(true);
    expect(profileBadge.text()).toBe("fast");
    expect(header.find('[data-test="chat-profile-badge"]').exists()).toBe(false);
  });

  it("shows current session worktree and branch metadata in the composer", async () => {
    const wrapper = mountChatPanel((session) => {
      session.sessions = [
        {
          id: "ses_1",
          title: "Project session",
          profile: "fast",
          project_id: "project_1",
          worktree_path: "/repo/.worktrees/project-chat",
          branch: "feat/project-chat",
          visibility: null
        }
      ];
    });
    await flushPromises();

    const gitMeta = wrapper.find('[data-test="session-git-meta"]');
    expect(gitMeta.exists()).toBe(true);
    expect(gitMeta.text()).toContain("/repo/.worktrees/project-chat");
    expect(gitMeta.text()).toContain("feat/project-chat");
  });

  it("renders context meter as a ring inside the composer input row", async () => {
    const wrapper = mountChatPanel((session) => {
      session.lastContextUsage = makeUsage();
    });
    await flushPromises();

    const inputRow = wrapper.find(".input-row");
    expect(inputRow.find('[data-test="context-meter-ring"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="context-meter-bar"]').exists()).toBe(false);
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

  it("audit anchors: exposes stable chat pilot selectors", async () => {
    const wrapper = mountChatPanel((session) => {
      session.projection.messages = [
        { role: "user", content: "Hello" },
        { role: "assistant", content: "Hi" },
        { role: "assistant", content: "[error] network failed" }
      ];
      session.projection.token_stream = "Streaming";
      session.isStreaming = true;
    });
    await flushPromises();

    expect(wrapper.find('[data-test="chat-message"][data-role="user"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="chat-message"][data-role="assistant"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="stream-indicator"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="error-banner"]').exists()).toBe(true);
  });

  it("audit anchors: exposes stable empty chat pilot selector", async () => {
    const wrapper = mountChatPanel((session) => {
      session.projection.messages = [];
      session.projection.token_stream = "";
      session.lastSendError = null;
      session.isStreaming = false;
    });
    await flushPromises();

    expect(wrapper.find('[data-test="chat-empty-state"]').exists()).toBe(true);
  });

  it("P1-S3-send-error: shows a visible send error banner", async () => {
    const wrapper = mountChatPanel((session) => {
      session.lastSendError = "model unavailable";
    });
    await flushPromises();

    const errorBanner = wrapper.find('[data-test="error-banner"]');
    expect(errorBanner.exists()).toBe(true);
    expect(errorBanner.text()).toContain("model unavailable");
  });
});
