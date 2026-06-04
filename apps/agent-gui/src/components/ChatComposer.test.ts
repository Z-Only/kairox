import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { flushPromises } from "@vue/test-utils";
import ChatComposer from "./ChatComposer.vue";
import chatComposerSource from "./ChatComposer.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

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
  default: { name: "CommandPalette", template: "<div/>", props: ["visible", "filterText"] }
}));
vi.mock("@/components/FileMentionPalette.vue", () => ({
  default: { name: "FileMentionPalette", template: "<div/>", props: ["visible", "filterText"] }
}));
vi.mock("@/composables/useNotifications", () => ({
  useNotifications: () => ({ notify: vi.fn() })
}));

import { useSessionStore } from "@/stores/session";
import { useProjectStore } from "@/stores/project";
import { invoke } from "@tauri-apps/api/core";

const mockedInvoke = vi.mocked(invoke);

function mountChatComposer() {
  const pinia = createPinia();
  setActivePinia(pinia);

  // Pre-configure session store before mount so onMounted's
  // loadProfileInfo() is blocked by the loadingProfileInfo guard.
  const session = useSessionStore();
  session.resetProjection();
  session.currentSessionId = "ses_1";
  session.currentProfile = "fast";
  session.isStreaming = false;
  session.profileInfos = [];
  session.loadingProfileInfo = true;

  const { wrapper } = mountWithPlugins(ChatComposer, {
    reusePinia: true,
    mount: {
      props: {
        workspacePath: "/mock/workspace",
        sessionGitMeta: []
      },
      global: {
        stubs: {
          AttachmentTray: true,
          ContextMeterPill: {
            template: '<div data-test="context-meter-pill" />'
          }
        }
      }
    }
  });

  return { wrapper, session };
}

beforeEach(() => {
  vi.clearAllMocks();
  mockedInvoke.mockImplementation(async (command) => {
    if (command === "refresh_config") return null;
    if (command === "get_profile_info") return useSessionStore().profileInfos;
    if (command === "list_skills") return [];
    if (command === "list_active_skills") return [];
    if (command === "set_session_approval_policy") return "always";
    if (command === "set_session_sandbox_policy") return '{"kind":"read_only"}';
    return null;
  });
});

describe("composer textarea chrome", () => {
  it("uses shared KxTextarea while preserving the message-input selector", () => {
    expectSourceMigration(chatComposerSource, {
      required: [
        "KxTextarea",
        'data-test="message-input"',
        "auto-resize",
        ':max-auto-resize-height="160"',
        'resize="none"'
      ],
      forbidden: [".message-input {", ".message-input:focus", ".message-input:disabled"]
    });
  });

  it("sends a draft flushed by the textarea change event before the send button click", async () => {
    const { wrapper } = mountChatComposer();
    const textarea = wrapper.find<HTMLTextAreaElement>('textarea[data-test="message-input"]');

    textarea.element.value = "post-turn click draft";
    await textarea.trigger("change");
    await wrapper.find('[data-test="send-button"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("send_message", {
      content: "post-turn click draft",
      attachments: []
    });
  });

  it("loads skills on mount so the command palette can activate discovered skills", async () => {
    mountChatComposer();
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("list_skills");
    expect(mockedInvoke).toHaveBeenCalledWith("list_active_skills");
  });
});

describe("legacy policy selector cleanup", () => {
  it("does not render the removed single-axis selector", () => {
    const { wrapper } = mountChatComposer();

    expect(chatComposerSource).not.toContain("ChatPermissionSelector");
    expect(wrapper.find('[data-test="chat-permission-trigger"]').exists()).toBe(false);
  });
});

describe("composer metadata", () => {
  it("renders model, policy, branch metadata below the input row with context on the right", async () => {
    const { wrapper, session } = mountChatComposer();
    session.lastContextUsage = {
      total_tokens: 12_000,
      budget_tokens: 180_000,
      context_window: 200_000,
      output_reservation: 20_000,
      by_source: [["history", 12_000]],
      estimator: "cl100k_base",
      corrected_by_real_usage: false
    };
    await wrapper.vm.$nextTick();

    const footer = wrapper.find(".composer-footer");
    const meta = wrapper.find(".composer-meta");
    const paletteContainer = wrapper.find(".palette-container");
    const inputRow = wrapper.find(".input-row");
    const contextPill = wrapper.find('[data-test="composer-context-meter-pill"]');

    expect(footer.exists()).toBe(true);
    expect(meta.exists()).toBe(true);
    expect(paletteContainer.exists()).toBe(true);
    expect(contextPill.exists()).toBe(true);
    expect(paletteContainer.element.compareDocumentPosition(inputRow.element)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(inputRow.element.compareDocumentPosition(footer.element)).toBe(
      Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(footer.element.compareDocumentPosition(contextPill.element)).toBe(
      Node.DOCUMENT_POSITION_CONTAINED_BY | Node.DOCUMENT_POSITION_FOLLOWING
    );
    expect(inputRow.find(".composer-meta").exists()).toBe(false);
    expect(inputRow.find('[data-test="context-meter-pill"]').exists()).toBe(false);
  });

  it("renders pending project branch control after approval and sandbox controls", async () => {
    const { wrapper, session } = mountChatComposer();
    const projectStore = useProjectStore();
    vi.spyOn(projectStore, "listProjectBranches").mockResolvedValue(["main", "feat/chat"]);
    session.currentSessionId = null;
    session.pendingSessionDraft = {
      kind: "project",
      projectId: "project_1",
      branch: "main"
    };
    await wrapper.vm.$nextTick();

    const meta = wrapper.find(".composer-meta");
    const branchSelector = wrapper.find('[data-test="project-branch-selector"]');
    const gitMeta = wrapper.find('[data-test="session-git-meta"]');

    expect(branchSelector.exists()).toBe(true);
    expect(meta.classes()).toContain("composer-meta--branch-picker");
    expect(gitMeta.text()).toContain("main");
    expect(meta.html().indexOf('data-test="chat-sandbox-trigger"')).toBeLessThan(
      meta.html().indexOf('data-test="session-git-meta"')
    );
  });
});

describe("approval policy selector", () => {
  it("renders the approval trigger with current policy label", () => {
    const { wrapper, session } = mountChatComposer();
    session.approvalPolicy = "on_request";

    const trigger = wrapper.find('[data-test="chat-approval-trigger"]');
    expect(trigger.exists()).toBe(true);
    expect(trigger.text()).toBe("On Request");
  });

  it("updates trigger label when approval policy changes", async () => {
    const { wrapper, session } = mountChatComposer();
    session.approvalPolicy = "never";
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="chat-approval-trigger"]').text()).toBe("Never");

    session.approvalPolicy = "always";
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="chat-approval-trigger"]').text()).toBe("Always");
  });

  it("has accessible aria-label reflecting current approval policy", async () => {
    const { wrapper, session } = mountChatComposer();
    session.approvalPolicy = "on_request";
    await wrapper.vm.$nextTick();

    const trigger = wrapper.find('[data-test="chat-approval-trigger"]');
    expect(trigger.attributes("aria-label")).toBe("Select approval policy. Current: On Request");
  });

  it("invokes set_session_approval_policy when an option is clicked", async () => {
    const { wrapper, session } = mountChatComposer();
    session.approvalPolicy = "on_request";
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="chat-approval-trigger"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="chat-approval-option-always"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("set_session_approval_policy", {
      approval: "always"
    });
    expect(session.approvalPolicy).toBe("always");
  });
});

describe("sandbox policy selector", () => {
  it("renders the sandbox trigger with current policy label parsed from JSON", async () => {
    const { wrapper, session } = mountChatComposer();
    session.sandboxPolicy = '{"kind":"read_only"}';
    await wrapper.vm.$nextTick();

    const trigger = wrapper.find('[data-test="chat-sandbox-trigger"]');
    expect(trigger.exists()).toBe(true);
    expect(trigger.text()).toBe("Read Only");
  });

  it("updates trigger label when sandbox policy changes", async () => {
    const { wrapper, session } = mountChatComposer();
    session.sandboxPolicy = '{"kind":"workspace_write","network_access":false,"writable_roots":[]}';
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="chat-sandbox-trigger"]').text()).toBe("Workspace Write");

    session.sandboxPolicy = '{"kind":"danger_full_access"}';
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="chat-sandbox-trigger"]').text()).toBe("Danger Full Access");
  });

  it("has accessible aria-label reflecting current sandbox policy", async () => {
    const { wrapper, session } = mountChatComposer();
    session.sandboxPolicy = '{"kind":"read_only"}';
    await wrapper.vm.$nextTick();

    const trigger = wrapper.find('[data-test="chat-sandbox-trigger"]');
    expect(trigger.attributes("aria-label")).toBe("Select sandbox policy. Current: Read Only");
  });

  it("invokes set_session_sandbox_policy with canonical JSON when an option is clicked", async () => {
    const targetJson = '{"kind":"read_only"}';
    const { wrapper, session } = mountChatComposer();
    session.sandboxPolicy = '{"kind":"workspace_write","network_access":false,"writable_roots":[]}';
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="chat-sandbox-trigger"]').trigger("click");
    await wrapper.vm.$nextTick();
    await wrapper.find('[data-test="chat-sandbox-option-read_only"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("set_session_sandbox_policy", {
      sandboxJson: targetJson
    });
    expect(session.sandboxPolicy).toBe(targetJson);
  });
});

describe("model reasoning selector", () => {
  it("refreshes the current config context when opening the model selector", async () => {
    const { wrapper, session } = mountChatComposer();
    session.profileInfos = [
      {
        alias: "fast",
        provider: "openai",
        model_id: "gpt-4o-mini",
        local: false,
        has_api_key: true,
        supports_reasoning: false
      }
    ];

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");
    await flushPromises();

    expect(mockedInvoke).toHaveBeenCalledWith("refresh_config");
    expect(mockedInvoke).toHaveBeenCalledWith("get_profile_info");
  });

  it("removes the model selector popover after choosing a model", async () => {
    const { wrapper, session } = mountChatComposer();
    session.currentProfile = "fast";
    session.profileInfos = [
      {
        alias: "fast",
        provider: "fake",
        model_id: "fake-model",
        local: true,
        has_api_key: false,
        supports_reasoning: false
      },
      {
        alias: "smart",
        provider: "ali-mo",
        model_id: "claude-opus-4-6",
        local: false,
        has_api_key: true,
        supports_reasoning: false
      }
    ];
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");
    expect(wrapper.find('[data-test="chat-model-popover"]').exists()).toBe(true);

    await wrapper.find('[data-test="chat-model-option-smart"]').trigger("click");
    await flushPromises();

    expect(session.currentProfile).toBe("smart");
    expect(wrapper.find('[data-test="chat-model-popover"]').exists()).toBe(false);
  });

  it("shows reasoning levels when hovering a reasoning-capable model", async () => {
    const { wrapper, session } = mountChatComposer();
    session.currentProfile = "smart";
    session.currentReasoningEffort = "middle";
    session.profileInfos = [
      {
        alias: "fast",
        provider: "openai",
        model_id: "gpt-4o-mini",
        local: false,
        has_api_key: true,
        supports_reasoning: false
      },
      {
        alias: "smart",
        provider: "openai",
        model_id: "gpt-5.2",
        local: false,
        has_api_key: true,
        supports_reasoning: true
      }
    ];
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-model-option-smart"]').trigger("mouseenter");
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="chat-model-popover"]').classes()).toContain(
      "chat-model-popover-panel"
    );
    expect(wrapper.find('[data-test="chat-model-option-smart"]').classes()).toContain(
      "kx-popover-option"
    );
    const popover = wrapper.find('[data-test="chat-model-popover"]');
    const layout = wrapper.find(".chat-model-popover-layout");
    const reasoningPanel = wrapper.find('[data-test="chat-reasoning-panel"]');
    expect(reasoningPanel.exists()).toBe(true);
    expect(layout.exists()).toBe(true);
    expect(reasoningPanel.element.parentElement).toBe(layout.element);
    expect(Array.from(layout.element.children).map((child) => child.className)).toEqual([
      "chat-model-card",
      "chat-reasoning-panel chat-reasoning-panel--anchored"
    ]);
    expect(popover.classes()).toContain("chat-model-popover-panel");
    expect(wrapper.find('[data-test="chat-reasoning-option-middle"]').classes()).toContain(
      "selected"
    );
    expect(wrapper.find('[data-test="chat-reasoning-custom-input"]').exists()).toBe(true);
  });

  it("shows reasoning levels for Claude profiles marked reasoning-capable by metadata", async () => {
    const { wrapper, session } = mountChatComposer();
    session.currentProfile = "claude";
    session.currentReasoningEffort = null;
    session.profileInfos = [
      {
        alias: "claude",
        provider: "anthropic",
        model_id: "claude-sonnet-4-20250514",
        local: false,
        has_api_key: true,
        supports_reasoning: true
      }
    ];
    await wrapper.vm.$nextTick();

    const trigger = wrapper.find('[data-test="chat-model-trigger"]');
    expect(trigger.text()).toContain("Anthropic · Claude Sonnet 4 20250514");
    expect(trigger.text()).not.toContain("· low");

    await trigger.trigger("click");
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="chat-reasoning-panel"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="chat-reasoning-option-low"]').classes()).not.toContain(
      "selected"
    );
  });

  it("hides reasoning levels when hovering a non-reasoning model", async () => {
    const { wrapper, session } = mountChatComposer();
    session.currentProfile = "smart";
    session.currentReasoningEffort = "high";
    session.profileInfos = [
      {
        alias: "fast",
        provider: "openai",
        model_id: "gpt-4o-mini",
        local: false,
        has_api_key: true,
        supports_reasoning: false
      },
      {
        alias: "smart",
        provider: "openai",
        model_id: "gpt-5.2",
        local: false,
        has_api_key: true,
        supports_reasoning: true
      }
    ];
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");
    expect(wrapper.find('[data-test="chat-reasoning-panel"]').exists()).toBe(true);

    await wrapper.find('[data-test="chat-model-option-fast"]').trigger("mouseenter");
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="chat-reasoning-panel"]').exists()).toBe(false);
  });

  it("switches to a custom reasoning effort from the hovered model", async () => {
    mockedInvoke.mockResolvedValueOnce(null);
    const { wrapper, session } = mountChatComposer();
    session.currentProfile = "fast";
    session.currentReasoningEffort = null;
    session.profileInfos = [
      {
        alias: "smart",
        provider: "openai",
        model_id: "gpt-5.2",
        local: false,
        has_api_key: true,
        supports_reasoning: true
      }
    ];
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="chat-model-trigger"]').trigger("click");
    await wrapper.find('[data-test="chat-model-option-smart"]').trigger("mouseenter");
    await wrapper.find('[data-test="chat-reasoning-custom-input"]').setValue("reasoning-max");
    await wrapper.find('[data-test="chat-reasoning-custom-apply"]').trigger("click");

    expect(mockedInvoke).toHaveBeenCalledWith("switch_model", {
      sessionId: "ses_1",
      profileAlias: "smart",
      reasoningEffort: "reasoning-max"
    });
  });

  it("uses shared popover option styling for approval choices", async () => {
    const { wrapper, session } = mountChatComposer();
    session.approvalPolicy = "on_request";
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="chat-approval-trigger"]').trigger("click");
    await wrapper.vm.$nextTick();

    const popover = wrapper.find('[data-test="chat-approval-popover"]');
    const option = wrapper.find('[data-test="chat-approval-option-on_request"]');
    expect(popover.classes()).toContain("chat-approval-popover-panel");
    expect(option.classes()).toContain("kx-popover-option");
    expect(option.classes()).toContain("kx-popover-option--selected");
  });
});

describe("conversation queue", () => {
  it("keeps the composer enabled while streaming and renders queued messages above the input", async () => {
    const { wrapper, session } = mountChatComposer();
    session.isStreaming = true;
    await wrapper.vm.$nextTick();

    const textarea = wrapper.find('textarea[data-test="message-input"]');
    expect(textarea.attributes("disabled")).toBeUndefined();

    await textarea.setValue("correct this before continuing");
    await textarea.trigger("keydown", { key: "Enter" });
    await wrapper.vm.$nextTick();

    expect(mockedInvoke).not.toHaveBeenCalledWith("send_message", expect.anything());
    expect(wrapper.find('[data-test="queued-message-list"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="queued-message-item"]').text()).toContain(
      "correct this before continuing"
    );
  });

  it("keeps send available during compaction and queues instead of sending immediately", async () => {
    const { wrapper, session } = mountChatComposer();
    session.compacting = true;
    await wrapper.vm.$nextTick();

    const textarea = wrapper.find('textarea[data-test="message-input"]');
    await textarea.setValue("send after compact");
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="send-button"]').attributes("disabled")).toBeUndefined();

    await textarea.trigger("keydown", { key: "Enter" });
    await wrapper.vm.$nextTick();

    expect(mockedInvoke).not.toHaveBeenCalledWith("send_message", expect.anything());
    expect(wrapper.find('[data-test="queued-message-list"]').text()).toContain(
      "send after compact"
    );
  });

  it("clears all queued messages with one action without sending them", async () => {
    const { wrapper, session } = mountChatComposer();
    session.isStreaming = true;
    await wrapper.vm.$nextTick();

    const textarea = wrapper.find('textarea[data-test="message-input"]');
    await textarea.setValue("first queued");
    await textarea.trigger("keydown", { key: "Enter" });
    await wrapper.vm.$nextTick();
    await textarea.setValue("second queued");
    await textarea.trigger("keydown", { key: "Enter" });
    await wrapper.vm.$nextTick();

    expect(wrapper.findAll('[data-test="queued-message-item"]')).toHaveLength(2);

    const clearButton = wrapper.find('[data-test="queued-message-clear"]');
    expect(clearButton.attributes("aria-label")).toBe("Clear queued messages");
    expect(clearButton.text()).toBe("Clear all");
    await clearButton.trigger("click");
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="queued-message-list"]').exists()).toBe(false);
    expect(mockedInvoke).not.toHaveBeenCalledWith("send_message", expect.anything());
  });

  it("renders queued messages in a capped-height fixed-row scroller", () => {
    expectSourceMigration(chatComposerSource, {
      required: [
        "KxActionButton",
        "max-height: var(--queued-message-list-max-height",
        "overflow-y: auto",
        "--queued-message-row-height",
        "height: var(--queued-message-row-height",
        "-webkit-line-clamp: 1"
      ],
      forbidden: [".queued-message-action {"]
    });
  });

  it("supports edit, guide-send, delete, drag sorting, and leaves ArrowUp to the textarea", async () => {
    const { wrapper, session } = mountChatComposer();
    session.isStreaming = true;
    await wrapper.vm.$nextTick();

    const textarea = wrapper.find('textarea[data-test="message-input"]');
    await textarea.setValue("queued draft");
    await textarea.trigger("keydown", { key: "Enter" });
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="queued-message-edit"]').trigger("click");
    await wrapper.vm.$nextTick();
    expect(wrapper.find('textarea[data-test="message-input"]').element).toHaveProperty(
      "value",
      "queued draft"
    );

    await wrapper.find('textarea[data-test="message-input"]').setValue("queued again");
    await wrapper.find('textarea[data-test="message-input"]').trigger("keydown", { key: "Enter" });
    await wrapper.vm.$nextTick();
    await wrapper.find('textarea[data-test="message-input"]').setValue("drag first");
    await wrapper.find('textarea[data-test="message-input"]').trigger("keydown", { key: "Enter" });
    await wrapper.vm.$nextTick();
    await wrapper.find('textarea[data-test="message-input"]').setValue("drag second");
    await wrapper.find('textarea[data-test="message-input"]').trigger("keydown", { key: "Enter" });
    await wrapper.vm.$nextTick();

    const queuedItems = wrapper.findAll('[data-test="queued-message-item"]');
    await queuedItems[2].trigger("dragstart");
    await queuedItems[1].trigger("drop");
    await wrapper.vm.$nextTick();
    expect(wrapper.findAll('[data-test="queued-message-item"]').map((item) => item.text())).toEqual(
      [
        expect.stringContaining("queued again"),
        expect.stringContaining("drag second"),
        expect.stringContaining("drag first")
      ]
    );

    await wrapper.find('[data-test="queued-message-guide"]').trigger("click");
    expect(mockedInvoke).toHaveBeenCalledWith("send_message", {
      content: "queued again",
      attachments: []
    });

    await wrapper.find('textarea[data-test="message-input"]').setValue("delete me");
    await wrapper.find('textarea[data-test="message-input"]').trigger("keydown", { key: "Enter" });
    await wrapper.vm.$nextTick();
    const deleteButtons = wrapper.findAll('[data-test="queued-message-delete"]');
    await deleteButtons.at(-1)?.trigger("click");
    await wrapper.vm.$nextTick();
    expect(wrapper.find('[data-test="queued-message-list"]').text()).not.toContain("delete me");

    await wrapper.find('textarea[data-test="message-input"]').setValue("arrow restore");
    await wrapper.find('textarea[data-test="message-input"]').trigger("keydown", { key: "Enter" });
    await wrapper.vm.$nextTick();
    await wrapper
      .find('textarea[data-test="message-input"]')
      .trigger("keydown", { key: "ArrowUp" });
    await wrapper.vm.$nextTick();
    expect(wrapper.find('textarea[data-test="message-input"]').element).toHaveProperty("value", "");
    expect(wrapper.find('[data-test="queued-message-list"]').text()).toContain("arrow restore");
  });
});
