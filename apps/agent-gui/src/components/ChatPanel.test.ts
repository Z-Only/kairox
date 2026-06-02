import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import ChatPanel from "./ChatPanel.vue";
import chatPanelSource from "./ChatPanel.vue?raw";
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

// Attaching the wrapper to `document.body` is required for the keyboard-
// navigation specs: jsdom only routes `.focus()` and `document.activeElement`
// for elements that live in the document tree. Detached wrappers focus into
// the void, which surfaced as "expected items[0], received <body>" failures
// before the attachment was added. The other specs are unaffected because
// they query through `wrapper.find(...)`, which is scoped to the mounted
// root regardless of attachment.
const mountedWrappers: Array<{ unmount: () => void }> = [];

function mountChatPanel(prepareSession?: (session: ReturnType<typeof useSessionStore>) => void) {
  const { wrapper } = mountWithPlugins(ChatPanel, {
    initialRoute: "/workbench",
    mount: { attachTo: document.body }
  });
  mountedWrappers.push(wrapper);
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

afterEach(() => {
  // Unmount every wrapper attached this turn so document.body is clean for
  // the next spec — important for `document.activeElement` assertions.
  while (mountedWrappers.length > 0) {
    const wrapper = mountedWrappers.pop();
    try {
      wrapper?.unmount();
    } catch {
      // Best-effort cleanup; a failed unmount must not mask the real failure.
    }
  }
  document.body.innerHTML = "";
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

  it("keeps message bubble styles scoped through ChatMessageItem internals", () => {
    expectSourceMigration(chatPanelSource, {
      required: [
        ".message :deep(.message-content)",
        ".message-user :deep(.message-content)",
        ".message-assistant :deep(.message-content)"
      ],
      forbiddenPatterns: [
        /^\.message-content\s*\{/m,
        /^\.message-user\s+\.message-content/m,
        /^\.message-assistant\s+\.message-content/m
      ]
    });
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

  it("shows only the branch as git metadata for a main project workspace", async () => {
    const wrapper = mountChatPanel((session) => {
      session.sessions = [
        {
          id: "ses_1",
          title: "Project session",
          profile: "fast",
          project_id: "project_1",
          worktree_path: "/repo",
          branch: "main",
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
    expect(gitMeta.text()).toBe("main");
    expect(gitMeta.text()).not.toContain("worktree");
    expect(wrapper.text()).not.toContain("/repo");
  });

  it("resolves missing branch metadata without exposing the project root path", async () => {
    mockedInvoke.mockImplementation(async (command) => {
      if (command === "get_profile_info") return [];
      if (command === "get_project_instruction_summary") {
        return { source_paths: [], warning: null };
      }
      if (command === "get_session_git_status") {
        return {
          kind: "clean",
          branch: "main",
          worktree_path: "/repo",
          message: null
        };
      }
      return undefined;
    });
    const wrapper = mountChatPanel((session) => {
      session.sessions = [
        {
          id: "ses_1",
          title: "Project session",
          profile: "fast",
          project_id: "project_1",
          worktree_path: "/repo",
          branch: null,
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
    expect(mockedInvoke).toHaveBeenCalledWith("get_session_git_status", {
      sessionId: "ses_1"
    });
    expect(gitMeta.exists()).toBe(true);
    expect(gitMeta.text()).toBe("main");
    expect(wrapper.text()).not.toContain("/repo");
  });

  it("shows worktree and branch without exposing a worktree path", async () => {
    const wrapper = mountChatPanel((session) => {
      session.sessions = [
        {
          id: "ses_1",
          title: "Project worktree session",
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
    expect(gitMeta.text()).not.toContain("/repo/.kairox/worktrees/project-chat");
  });

  it("keeps model selector and git metadata stable with long labels", () => {
    expectSourceMigration(chatComposerSource, {
      requiredPatterns: [
        /\.composer-meta\s*\{[\s\S]*min-width:\s*0/,
        /\.composer-meta\s*\{[\s\S]*overflow:\s*hidden/,
        /\.composer-meta--branch-picker\s*\{[\s\S]*overflow:\s*visible/,
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
    // (`ChatCompactionItem`, PRs #471-#477). The compact footer pill may
    // live near the composer, but the input row itself should stay free of
    // any heavy `<ContextMeter>` mount in either ring or bar variant.
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

  it("passes tool start timestamps through to the inline tool-call item", async () => {
    const wrapper = mountChatPanel();
    const trace = useTraceStore();
    trace.entries.push({
      id: "tool_timing_1",
      kind: "tool",
      status: "completed",
      title: "shell exec",
      toolId: "shell",
      startedAt: Date.now() - 3000,
      durationMs: 1200,
      expanded: false
    } as TraceEntryData);
    await flushPromises();

    const toolItem = wrapper
      .findAll('[data-test="chat-tool-call-item"]')
      .find((item) => item.text().includes("shell exec"));
    expect(toolItem).toBeTruthy();

    let startedAgo = toolItem!.find('[data-test="chat-tool-call-started-ago"]');
    if (!startedAgo.exists()) {
      await toolItem!.find('[data-test="chat-tool-call-toggle"]').trigger("click");
      await flushPromises();
      startedAgo = toolItem!.find('[data-test="chat-tool-call-started-ago"]');
    }
    expect(startedAgo.exists(), toolItem!.html()).toBe(true);
    expect(startedAgo.text()).toMatch(/^started [34]s ago$/);
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

  it("does not render the removed header worktree-session action", async () => {
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
    await router.isReady();
    await flushPromises();

    expect(wrapper.find('[data-test="project-worktree-session-trigger"]').exists()).toBe(false);
    expect(wrapper.find('[data-test="project-worktree-branch-input"]').exists()).toBe(false);
  });

  it("shows a searchable branch selector for a pending project placeholder", async () => {
    const { wrapper } = mountWithPlugins(ChatPanel, {
      initialRoute: "/workbench"
    });
    const session = useSessionStore();
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
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main", "feat/chat"]);
    await session.startProjectDraftSession("project_1");
    await flushPromises();

    expect(wrapper.find('[data-test="project-branch-selector"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="session-git-meta"]').text()).toContain("main");

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="project-branch-search"]').setValue("feat");
    await flushPromises();
    expect(wrapper.find('[data-test="project-branch-option-feat-chat"]').exists()).toBe(true);

    await wrapper.find('[data-test="project-branch-option-feat-chat"]').trigger("click");
    await flushPromises();
    expect(session.currentSessionInfo?.branch).toBe("feat/chat");

    await wrapper.find('[data-test="session-git-meta"]').trigger("click");
    await flushPromises();
    await wrapper.find('[data-test="project-branch-search"]').setValue("feat/new-chat");
    await flushPromises();
    await wrapper.find('[data-test="project-branch-create"]').trigger("click");
    expect(session.currentSessionInfo?.branch).toBe("feat/new-chat");
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

  describe("keyboard navigation between chat-stream items", () => {
    /**
     * Test helper: queue three chat-stream items (a user message, an
     * assistant message, and a pending permission) so the panel renders
     * three focusable `[data-chat-stream-item]` rows. Returns the rendered
     * elements in document order for direct focus assertions.
     */
    async function mountThreeItems(): Promise<{
      wrapper: ReturnType<typeof mountChatPanel>;
      items: HTMLElement[];
      panel: HTMLElement;
    }> {
      const wrapper = mountChatPanel((session) => {
        session.projection.messages = [
          { role: "user", content: "first" },
          { role: "assistant", content: "second" }
        ];
        // Bind the active session to a project so the worktree-form trigger
        // (used by the "<input> ignores j" spec) is rendered. Other specs
        // tolerate the extra header button — it doesn't add a stream item.
        session.sessions = [
          {
            id: "ses_1",
            title: "Project session",
            profile: "fast",
            project_id: "project_kbd",
            worktree_path: "/repo",
            branch: "main",
            visibility: "draft_hidden"
          }
        ];
      });
      const projectStore = useProjectStore();
      projectStore.projects = [
        {
          projectId: "project_kbd",
          displayName: "Kbd Project",
          rootPath: "/repo",
          removedAt: null,
          sortOrder: 0,
          expanded: true,
          pathExists: true
        }
      ];
      const trace = useTraceStore();
      trace.entries.push({
        id: "perm_kbd_1",
        kind: "permission",
        status: "pending",
        title: "Allow rm /tmp/k?",
        toolId: "shell",
        startedAt: 1,
        expanded: false
      } as TraceEntryData);
      await flushPromises();

      const panelEl = wrapper.find('[data-test="chat-panel"]').element as HTMLElement;
      const items = Array.from(panelEl.querySelectorAll<HTMLElement>("[data-chat-stream-item]"));
      return { wrapper, items, panel: panelEl };
    }

    function dispatchKey(
      target: HTMLElement,
      key: string,
      init: Partial<KeyboardEventInit> = {}
    ): KeyboardEvent {
      const event = new KeyboardEvent("keydown", {
        key,
        bubbles: true,
        cancelable: true,
        ...init
      });
      target.dispatchEvent(event);
      return event;
    }

    it("renders a [data-chat-stream-item] focusable wrapper per stream item", async () => {
      const { items } = await mountThreeItems();
      expect(items).toHaveLength(3);
      for (const el of items) {
        expect(el.tabIndex).toBe(0);
      }
    });

    it("j moves focus from item 0 to item 1; k moves back", async () => {
      const { items, panel } = await mountThreeItems();
      items[0].focus();
      expect(document.activeElement).toBe(items[0]);

      dispatchKey(panel, "j");
      expect(document.activeElement).toBe(items[1]);

      dispatchKey(panel, "k");
      expect(document.activeElement).toBe(items[0]);
    });

    it("ArrowDown behaves like j and ArrowUp like k", async () => {
      const { items, panel } = await mountThreeItems();
      items[0].focus();

      dispatchKey(panel, "ArrowDown");
      expect(document.activeElement).toBe(items[1]);

      dispatchKey(panel, "ArrowUp");
      expect(document.activeElement).toBe(items[0]);
    });

    it("clamps at the last item when pressing j past the end", async () => {
      const { items, panel } = await mountThreeItems();
      items[items.length - 1].focus();
      dispatchKey(panel, "j");
      expect(document.activeElement).toBe(items[items.length - 1]);
    });

    it("clamps at the first item when pressing k past the start", async () => {
      const { items, panel } = await mountThreeItems();
      items[0].focus();
      dispatchKey(panel, "k");
      expect(document.activeElement).toBe(items[0]);
    });

    it("Enter on a focused permission item triggers the allow click", async () => {
      const { panel } = await mountThreeItems();
      const permissionItem = panel
        .querySelector<HTMLElement>('[data-test="chat-permission-item"]')
        ?.closest<HTMLElement>("[data-chat-stream-item]");
      expect(permissionItem).not.toBeNull();
      if (!permissionItem) return;
      permissionItem.focus();

      const allowButton = permissionItem.querySelector<HTMLElement>(
        '[data-test="permission-allow"]'
      );
      expect(allowButton).not.toBeNull();
      const clickSpy = vi.fn();
      allowButton!.addEventListener("click", clickSpy);

      dispatchKey(permissionItem, "Enter");
      expect(clickSpy).toHaveBeenCalledTimes(1);
    });

    it("Enter on a focused tool-call item toggles its detail row", async () => {
      const wrapper = mountChatPanel();
      const trace = useTraceStore();
      trace.entries.push({
        id: "tool_kbd_1",
        kind: "tool",
        status: "completed",
        title: "shell exec",
        toolId: "keyboard_nav_tool",
        startedAt: 1,
        durationMs: 1200,
        input: "echo hi",
        expanded: false
      } as TraceEntryData);
      await flushPromises();

      const panel = wrapper.find('[data-test="chat-panel"]').element as HTMLElement;
      const item = panel.querySelector<HTMLElement>("[data-chat-stream-item]");
      expect(item).not.toBeNull();
      const wasExpanded = wrapper.find(".chat-tool-call__detail").exists();

      item!.focus();
      dispatchKey(panel, "Enter");
      await flushPromises();

      expect(wrapper.find(".chat-tool-call__detail").exists()).toBe(!wasExpanded);
    });

    it("gg jumps focus to the first item, G to the last", async () => {
      const { items, panel } = await mountThreeItems();
      items[1].focus();

      // Two `g` presses in quick succession should land on the first item.
      dispatchKey(panel, "g");
      dispatchKey(panel, "g");
      expect(document.activeElement).toBe(items[0]);

      dispatchKey(panel, "G", { shiftKey: true });
      expect(document.activeElement).toBe(items[items.length - 1]);
    });

    it("a single g press without a follow-up does not move focus", async () => {
      const { items, panel } = await mountThreeItems();
      items[1].focus();
      dispatchKey(panel, "g");
      expect(document.activeElement).toBe(items[1]);
    });

    it("ignores j/k when focus is inside the composer textarea", async () => {
      const { wrapper, items } = await mountThreeItems();
      items[0].focus();
      expect(document.activeElement).toBe(items[0]);

      const textarea = wrapper.find('textarea[data-test="message-input"]')
        .element as HTMLTextAreaElement;
      textarea.focus();
      expect(document.activeElement).toBe(textarea);

      dispatchKey(textarea, "j");
      // Focus must remain on the textarea — the panel handler must NOT
      // hijack `j` while the composer is editing.
      expect(document.activeElement).toBe(textarea);
    });

    it("ignores keys when focus is inside a generic <input>", async () => {
      const { items, panel } = await mountThreeItems();
      const input = document.createElement("input");
      input.setAttribute("data-test", "generic-keyboard-input");
      panel.appendChild(input);
      input.focus();
      expect(document.activeElement).toBe(input);

      dispatchKey(panel, "j");
      // Focus must remain on the input — composer / form inputs own j/k.
      expect(document.activeElement).toBe(input);
      // Sanity: nav still works once focus returns to an item.
      items[0].focus();
      dispatchKey(panel, "j");
      expect(document.activeElement).toBe(items[1]);
    });

    it("does NOT intercept Ctrl+j or Cmd+k so host shortcuts pass through", async () => {
      const { items, panel } = await mountThreeItems();
      items[0].focus();

      const ctrlJ = dispatchKey(panel, "j", { ctrlKey: true });
      expect(ctrlJ.defaultPrevented).toBe(false);
      // Focus must remain on item 0 — Ctrl+j is reserved for the host.
      expect(document.activeElement).toBe(items[0]);

      const cmdK = dispatchKey(panel, "k", { metaKey: true });
      expect(cmdK.defaultPrevented).toBe(false);
      expect(document.activeElement).toBe(items[0]);

      const altG = dispatchKey(panel, "G", { altKey: true, shiftKey: true });
      expect(altG.defaultPrevented).toBe(false);
      expect(document.activeElement).toBe(items[0]);
    });

    it("does not render visible keyboard shortcut hints inside the chat panel", async () => {
      const { wrapper } = await mountThreeItems();
      expect(wrapper.find('[data-test="chat-keyboard-hint"]').exists()).toBe(false);
    });
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
