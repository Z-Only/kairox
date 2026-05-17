import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import ChatComposer from "./ChatComposer.vue";
import { mountWithPlugins } from "@/test-utils/mount";

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

describe("permission mode selector", () => {
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
