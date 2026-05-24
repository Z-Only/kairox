import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import ChatPanel from "./ChatPanel.vue";
import chatComposerSource from "./ChatComposer.vue?raw";
import chatModelSelectorSource from "./ChatModelSelector.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

// jsdom does not implement `Element.prototype.scrollTo`. The scrollbar
// calls it inside its own `scrollTo()` method when the message-list watcher
// fires (see ChatPanel.vue), which would surface as a noisy unhandled
// rejection during these tests even though no assertion depends on the
// scroll behaviour. Stub it once for the whole file.
if (typeof Element !== "undefined" && !Element.prototype.scrollTo) {
  Element.prototype.scrollTo = (() => {}) as Element["scrollTo"];
}
// jsdom likewise has no `Element.prototype.scrollIntoView`; ChatPanel's
// jump-to-pending-permission CTA invokes it on click. Provide a default
// stub so component renders don't crash; individual specs replace it with
// a spy when they need to assert the call.
if (typeof Element !== "undefined" && !Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = (() => {}) as Element["scrollIntoView"];
}
// jsdom does not implement `IntersectionObserver`. The jump-to-pending-
// permission CTA uses it to decide whether the first pending permission's
// DOM node is in the visible scroll region; with no native impl the
// component falls back to "not yet observed", which is the same state the
// specs assert against.
if (typeof globalThis.IntersectionObserver === "undefined") {
  class StubIntersectionObserver {
    observe(): void {}
    unobserve(): void {}
    disconnect(): void {}
    takeRecords(): IntersectionObserverEntry[] {
      return [];
    }
    root: Element | null = null;
    rootMargin = "";
    thresholds: ReadonlyArray<number> = [];
  }
  (
    globalThis as unknown as {
      IntersectionObserver: typeof IntersectionObserver;
    }
  ).IntersectionObserver = StubIntersectionObserver as unknown as typeof IntersectionObserver;
}

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(() => Promise.resolve(vi.fn()))
}));
vi.mock("@/composables/useDraftStore", () => ({
  useDraftStore: () => ({
    loadDraft: vi.fn(() => Promise.resolve("")),
    saveDraft: vi.fn(() => Promise.resolve()),
    clearDraft: vi.fn(() => Promise.resolve())
  })
}));
vi.mock("@/composables/useCommandRegistry", () => ({
  useCommandRegistry: () => ({
    filterText: { value: "" },
    setFilter: vi.fn(),
    allItems: () => []
  })
}));
vi.mock("@/components/CommandPalette.vue", () => ({
  default: {
    name: "CommandPalette",
    template: "<div/>",
    props: ["visible", "filterText"]
  }
}));
vi.mock("@/components/FileMentionPalette.vue", () => ({
  default: {
    name: "FileMentionPalette",
    template: "<div/>",
    props: ["visible", "filterText"]
  }
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import { useSessionStore } from "@/stores/session";
import { useProjectStore } from "@/stores/project";
import { useTraceStore } from "@/stores/trace";
import type { ContextUsage } from "@/types";
import type { TraceEntryData } from "@/types/trace";

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
  mockedInvoke.mockImplementation(async (command) => {
    if (command === "get_profile_info") return [];
    if (command === "get_project_instruction_summary") {
      return { source_paths: [], warning: null };
    }
    return undefined;
  });
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

  it("opens a model selector from the composer badge and marks the current model", async () => {
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "get_profile_info") {
        return [
          {
            alias: "fast",
            provider: "openai",
            model_id: "gpt-4o",
            local: false,
            has_api_key: true
          },
          {
            alias: "smart",
            provider: "anthropic",
            model_id: "claude-3-5-sonnet",
            local: false,
            has_api_key: true
          }
        ];
      }
      if (command === "get_project_instruction_summary") {
        return { source_paths: [], warning: null };
      }
      return undefined;
    });
    const wrapper = mountChatPanel((session) => {
      session.currentProfile = "fast";
      session.profileInfos = [
        {
          alias: "fast",
          provider: "openai",
          model_id: "gpt-4o",
          local: false,
          has_api_key: true
        },
        {
          alias: "smart",
          provider: "anthropic",
          model_id: "claude-3-5-sonnet",
          local: false,
          has_api_key: true
        }
      ] as never;
    });
    await flushPromises();

    const header = wrapper.find(".chat-header");
    const inputArea = wrapper.find(".input-area");
    const modelTrigger = inputArea.find('[data-test="chat-model-trigger"]');

    expect(modelTrigger.exists()).toBe(true);
    expect(modelTrigger.text()).toContain("OpenAI · GPT-4o");
    expect(header.find('[data-test="chat-model-trigger"]').exists()).toBe(false);

    await modelTrigger.trigger("click");
    await flushPromises();

    const popover = wrapper.find('[data-test="chat-model-popover"]');
    const currentOption = wrapper.find('[data-test="chat-model-option-fast"]');

    expect(popover.exists()).toBe(true);
    expect(popover.text()).toContain("OpenAI · GPT-4o");
    expect(popover.text()).toContain("Anthropic · Claude 3.5 Sonnet");
    expect(currentOption.attributes("aria-current")).toBe("true");
    expect(currentOption.text()).toContain("Current");
  });

  it("marks the displayed fallback model as current when the session profile is unavailable", async () => {
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "get_profile_info") {
        return [
          {
            alias: "deepseek",
            provider: "deepseek",
            model_id: "deepseek-chat",
            local: false,
            has_api_key: true
          },
          {
            alias: "fake",
            provider: "fake",
            model_id: "fake-model",
            local: true,
            has_api_key: true
          }
        ];
      }
      if (command === "get_project_instruction_summary") {
        return { source_paths: [], warning: null };
      }
      return undefined;
    });
    const wrapper = mountChatPanel((session) => {
      session.currentProfile = "deep";
      session.profileInfos = [
        {
          alias: "deepseek",
          provider: "deepseek",
          model_id: "deepseek-chat",
          local: false,
          has_api_key: true
        },
        {
          alias: "fake",
          provider: "fake",
          model_id: "fake-model",
          local: true,
          has_api_key: true
        }
      ] as never;
    });
    await flushPromises();

    const modelTrigger = wrapper.find('[data-test="chat-model-trigger"]');
    expect(modelTrigger.text()).toContain("Deepseek");

    await modelTrigger.trigger("click");
    await flushPromises();

    const fallbackOption = wrapper.find('[data-test="chat-model-option-deepseek"]');
    expect(fallbackOption.attributes("aria-current")).toBe("true");
    expect(fallbackOption.text()).toContain("Current");
  });

  it("shows current session worktree and branch metadata in the composer", async () => {
    const wrapper = mountChatPanel((session) => {
      session.sessions = [
        {
          id: "ses_1",
          title: "Project session",
          profile: "fast",
          project_id: "project_1",
          worktree_path: "/repo/.kairox/worktrees/project-chat",
          branch: "feat/project-chat",
          visibility: null
        }
      ];
    });
    const projectStore = useProjectStore();
    projectStore.projects = [
      {
        projectId: "project_1",
        displayName: "Kairox",
        rootPath: "/repo",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];
    await flushPromises();

    const gitMeta = wrapper.find('[data-test="session-git-meta"]');
    expect(gitMeta.exists()).toBe(true);
    expect(gitMeta.text()).toContain("worktree");
    expect(gitMeta.text()).toContain("feat/project-chat");
    expect(gitMeta.text().indexOf("worktree")).toBeLessThan(
      gitMeta.text().indexOf("feat/project-chat")
    );
  });

  it("keeps model selector and git metadata stable with long labels", () => {
    expectSourceMigration(chatComposerSource, {
      requiredPatterns: [
        /\.composer-meta\s*\{[\s\S]*min-width:\s*0/,
        /\.composer-meta\s*\{[\s\S]*overflow:\s*hidden/,
        /\.git-meta\s*\{[\s\S]*min-width:\s*0/,
        /\.git-meta\s*\{[\s\S]*max-width:\s*min\(100%,\s*420px\)/
      ]
    });
    expectSourceMigration(chatModelSelectorSource, {
      required: ['class="kx-popover-option__label chat-model-option-label"'],
      requiredPatterns: [
        /\.chat-model-trigger\s*\{[\s\S]*max-width:\s*min\(100%,\s*280px\)/,
        /\.chat-model-trigger\s*\{[\s\S]*overflow:\s*hidden/,
        /\.chat-model-trigger\s*\{[\s\S]*text-overflow:\s*ellipsis/,
        /\.chat-model-trigger\s*\{[\s\S]*white-space:\s*nowrap/
      ]
    });
  });

  it("no longer renders the primary ContextMeter ring inside the composer input row (R4-B demotion)", async () => {
    // R4-B moved the primary context-usage signal out of the chat
    // composer: compaction is now rendered inline in the chat stream
    // (`ChatCompactionItem`, PRs #471-#477) and the diagnostic meter is
    // surfaced via the demoted `ContextMeterPill` mounted in
    // `WorkbenchView`. The composer's input row should be free of any
    // `<ContextMeter>` mount in either ring or bar variant.
    const wrapper = mountChatPanel((session) => {
      session.lastContextUsage = makeUsage();
      session.projection.messages = [{ role: "user", content: "hi" }] as never;
    });
    await flushPromises();

    const inputRow = wrapper.find(".input-row");
    expect(inputRow.exists()).toBe(true);
    expect(inputRow.find('[data-test="context-meter-ring"]').exists()).toBe(false);
    expect(inputRow.find('[data-test="context-meter-bar"]').exists()).toBe(false);
    expect(inputRow.find('[data-test="context-meter"]').exists()).toBe(false);
  });

  it("does not surface any ContextMeter ring/bar in the chat panel even with no messages (R4-B demotion)", async () => {
    const wrapper = mountChatPanel();
    await flushPromises();

    expect(wrapper.find('[data-test="context-meter-ring"]').exists()).toBe(false);
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
    expect(wrapper.find('[data-test="send-button"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="send-button"]').text()).toBe("Queue");
  });

  it("shows Send button when not streaming", async () => {
    const wrapper = mountChatPanel((s) => {
      s.isStreaming = false;
    });
    await flushPromises();
    expect(wrapper.find('[data-test="send-button"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="cancel-button"]').exists()).toBe(false);
  });

  it("keeps the textarea enabled when isStreaming so messages can queue", async () => {
    const wrapper = mountChatPanel((s) => {
      s.isStreaming = true;
    });
    await flushPromises();
    // Assert via the native <textarea> element because that's what the
    // user actually interacts with. The data-test attribute lives on the
    // <textarea> itself (not a wrapper), so we select it directly.
    const textarea = wrapper.find('textarea[data-test="message-input"]');
    expect(textarea.exists()).toBe(true);
    expect(textarea.attributes("disabled")).toBeUndefined();
  });

  it("invokes cancel_session on Cancel click", async () => {
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

    const empty = wrapper.find('[data-test="chat-empty-state"]');
    expect(empty.exists()).toBe(true);
    expect(empty.classes()).toContain("kx-empty-state");
    expect(empty.classes()).toContain("kx-empty-state--section");
  });

  it("renders project instruction source filenames in an empty project chat", async () => {
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "get_profile_info") return [];
      if (command === "get_project_instruction_summary") {
        return {
          source_paths: ["/repo/AGENTS.md", "/repo/README.md"],
          warning: null
        };
      }
      return undefined;
    });
    const wrapper = mountChatPanel((session) => {
      session.projection.messages = [];
      session.projection.token_stream = "";
      session.lastSendError = null;
      session.isStreaming = false;
      session.sessions = [
        {
          id: "ses_1",
          title: "Project session",
          profile: "fast",
          project_id: "project_1",
          worktree_path: "/repo",
          branch: "main",
          visibility: "draft_hidden"
        }
      ];
    });
    await flushPromises();

    const projectStore = useProjectStore();
    const summary = wrapper.find('[data-test="project-instruction-summary"]');
    expect(mockedInvoke).toHaveBeenCalledWith("get_project_instruction_summary", {
      projectId: "project_1"
    });
    expect(projectStore.instructionSummariesByProject.get("project_1")?.sourcePaths).toEqual([
      "/repo/AGENTS.md",
      "/repo/README.md"
    ]);
    expect(summary.exists()).toBe(true);
    expect(summary.text()).toBe("Loaded AGENTS.md, README.md");
    expect(summary.text()).not.toContain("/repo/");
  });

  it("creates a worktree session from the active project chat", async () => {
    const { wrapper, router } = mountWithPlugins(ChatPanel, {
      initialRoute: "/workbench/ses_1"
    });
    const session = useSessionStore();
    const projectStore = useProjectStore();
    session.resetProjection();
    session.currentSessionId = "ses_1";
    session.currentProfile = "fast";
    session.sessions = [
      {
        id: "ses_1",
        title: "Project session",
        profile: "fast",
        project_id: "project_1",
        worktree_path: "/repo",
        branch: "main",
        visibility: "draft_hidden"
      }
    ];
    projectStore.projects = [
      {
        projectId: "project_1",
        displayName: "Kairox",
        rootPath: "/repo",
        removedAt: null,
        sortOrder: 0,
        expanded: true,
        pathExists: true
      }
    ];
    const createProjectWorktreeSession = vi
      .spyOn(projectStore, "createProjectWorktreeSession")
      .mockResolvedValue({
        sessionId: "wt_1",
        title: "New Session (feat-chat)",
        profile: "fast",
        projectId: "project_1",
        worktreePath: "/repo/.kairox/worktrees/feat-chat",
        branch: "feat-chat",
        visibility: "visible",
        deletedAt: null
      });
    vi.spyOn(session, "switchProjectSession").mockResolvedValue();
    await router.isReady();
    await flushPromises();

    await wrapper.find('[data-test="project-worktree-session-trigger"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="project-worktree-branch-input"]').setValue("feat-chat");
    await wrapper.find('[data-test="project-worktree-branch-confirm"]').trigger("click");
    await flushPromises();
    await router.isReady();

    expect(createProjectWorktreeSession).toHaveBeenCalledWith("project_1", "feat-chat");
    expect(session.switchProjectSession).toHaveBeenCalledWith(
      expect.objectContaining({ sessionId: "wt_1", branch: "feat-chat" })
    );
    expect(router.currentRoute.value.params.sessionId).toBe("wt_1");
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

  describe("jump-to-pending-permission CTA", () => {
    function pendingPermissionEntry(
      overrides: Partial<TraceEntryData> & { id: string }
    ): TraceEntryData {
      return {
        kind: "permission",
        status: "pending",
        title: "Allow shell?",
        toolId: "shell",
        startedAt: 0,
        expanded: false,
        ...overrides
      };
    }

    it("renders the floating CTA when an unresolved permission is in the chat stream", async () => {
      const wrapper = mountChatPanel();
      const trace = useTraceStore();
      trace.entries.push(
        pendingPermissionEntry({
          id: "perm_jump_1",
          title: "Allow rm /tmp/x?"
        })
      );
      await flushPromises();

      const cta = wrapper.find('[data-test="jump-pending-permission-cta"]');
      expect(cta.exists()).toBe(true);
      // Localised label embeds the unresolved permission count.
      expect(cta.text()).toContain("1");
    });

    it("hides the floating CTA when there are no pending permissions", async () => {
      const wrapper = mountChatPanel();
      await flushPromises();

      expect(wrapper.find('[data-test="jump-pending-permission-cta"]').exists()).toBe(false);
    });

    it("scrolls the first pending permission into view when clicked", async () => {
      const scrollIntoViewSpy = vi.fn();
      const originalScrollIntoView = Element.prototype.scrollIntoView;
      Element.prototype.scrollIntoView = scrollIntoViewSpy as Element["scrollIntoView"];
      try {
        const wrapper = mountChatPanel();
        const trace = useTraceStore();
        trace.entries.push(
          pendingPermissionEntry({
            id: "perm_jump_click",
            title: "Allow rm /tmp/y?"
          })
        );
        await flushPromises();

        const cta = wrapper.find('[data-test="jump-pending-permission-cta"]');
        expect(cta.exists()).toBe(true);
        await cta.trigger("click");

        expect(scrollIntoViewSpy).toHaveBeenCalledTimes(1);
        const callArg = scrollIntoViewSpy.mock.calls[0]?.[0];
        expect(callArg).toEqual({ behavior: "smooth", block: "center" });
      } finally {
        Element.prototype.scrollIntoView = originalScrollIntoView;
      }
    });
  });
});
