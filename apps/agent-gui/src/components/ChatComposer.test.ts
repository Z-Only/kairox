import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import ChatComposer from "./ChatComposer.vue";
import chatComposerSource from "./ChatComposer.vue?raw";
import chatPermissionSelectorSource from "./ChatPermissionSelector.vue?raw";
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
          ContextMeter: true,
          AttachmentTray: true
        }
      }
    }
  });

  return { wrapper, session };
}

beforeEach(() => {
  vi.clearAllMocks();
});

describe("composer textarea chrome", () => {
  it("uses shared KxTextarea while preserving the message-input selector", () => {
    expectSourceMigration(chatComposerSource, {
      required: ["KxTextarea", 'data-test="message-input"'],
      forbidden: [".message-input {", ".message-input:focus", ".message-input:disabled"]
    });
  });
});

describe("permission mode selector", () => {
  it("does not keep standard permission labels inline in the selector source", () => {
    expectSourceMigration(chatPermissionSelectorSource, {
      forbidden: ["Read Only", "Suggest", "Agent", "Autonomous", "Interactive"]
    });
  });

  it("renders the permission trigger button with current mode label", () => {
    const { wrapper, session } = mountChatComposer();
    session.permissionMode = "suggest";

    const trigger = wrapper.find('[data-test="chat-permission-trigger"]');
    expect(trigger.exists()).toBe(true);
    expect(trigger.text()).toBe("Suggest");
  });

  it("updates trigger label when session permissionMode changes", async () => {
    const { wrapper, session } = mountChatComposer();
    session.permissionMode = "read_only";
    await wrapper.vm.$nextTick();

    const trigger = wrapper.find('[data-test="chat-permission-trigger"]');
    expect(trigger.text()).toBe("Read Only");

    session.permissionMode = "autonomous";
    await wrapper.vm.$nextTick();
    expect(trigger.text()).toBe("Autonomous");
  });

  it("sets the default permission mode to suggest", () => {
    const { session } = mountChatComposer();
    expect(session.permissionMode).toBe("suggest");
  });

  it("falls back to raw mode string for unknown mode values", async () => {
    const { wrapper, session } = mountChatComposer();
    session.permissionMode = "custom_mode";
    await wrapper.vm.$nextTick();

    const trigger = wrapper.find('[data-test="chat-permission-trigger"]');
    expect(trigger.text()).toBe("custom_mode");
  });

  it("renders all five standard mode labels correctly", async () => {
    const { wrapper, session } = mountChatComposer();

    const modeLabels: Record<string, string> = {
      read_only: "Read Only",
      suggest: "Suggest",
      agent: "Agent",
      autonomous: "Autonomous",
      interactive: "Interactive"
    };

    for (const [value, label] of Object.entries(modeLabels)) {
      session.permissionMode = value;
      await wrapper.vm.$nextTick();
      const trigger = wrapper.find('[data-test="chat-permission-trigger"]');
      expect(trigger.text()).toBe(label);
    }
  });

  it("has accessible aria-label reflecting current permission mode", async () => {
    const { wrapper, session } = mountChatComposer();
    session.permissionMode = "agent";
    await wrapper.vm.$nextTick();

    const trigger = wrapper.find('[data-test="chat-permission-trigger"]');
    expect(trigger.attributes("aria-label")).toBe("Select permission level. Current: Agent");
  });
});

describe("model reasoning selector", () => {
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
    expect(wrapper.find('[data-test="chat-reasoning-panel"]').exists()).toBe(true);
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
    expect(trigger.text()).toContain("Anthropic · Claude Sonnet 4 20250514 · low");

    await trigger.trigger("click");
    await wrapper.vm.$nextTick();

    expect(wrapper.find('[data-test="chat-reasoning-panel"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="chat-reasoning-option-low"]').classes()).toContain("selected");
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

  it("uses shared popover option styling for permission choices", async () => {
    const { wrapper, session } = mountChatComposer();
    session.permissionMode = "suggest";
    await wrapper.vm.$nextTick();

    await wrapper.find('[data-test="chat-permission-trigger"]').trigger("click");
    await wrapper.vm.$nextTick();

    const popover = wrapper.find('[data-test="chat-permission-popover"]');
    const option = wrapper.find('[data-test="chat-permission-option-suggest"]');
    expect(popover.classes()).toContain("chat-permission-popover-panel");
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
