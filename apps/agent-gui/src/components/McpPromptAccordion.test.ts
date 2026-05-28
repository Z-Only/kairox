import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import type { McpPromptDefResponse } from "@/generated/commands";
import { useMcpStore } from "@/stores/mcp";
import McpPromptAccordion from "./McpPromptAccordion.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("@/generated/commands", () => ({
  commands: {
    listMcpPrompts: vi.fn(),
    listMcpResources: vi.fn(),
    readMcpResource: vi.fn(),
    listMcpServerSettings: vi.fn(),
    getEffectiveMcpServers: vi.fn(),
    checkMcpHealth: vi.fn(),
    getMcpToolStates: vi.fn(),
    testMcpConnectivity: vi.fn()
  }
}));

const SERVER_ID = "test-mcp-server";

function makePrompt(overrides: Partial<McpPromptDefResponse> = {}): McpPromptDefResponse {
  return {
    name: "summarize",
    description: "Summarize a document",
    argument_count: 2,
    ...overrides
  };
}

function mountAccordion(serverId = SERVER_ID) {
  return mountWithPlugins(McpPromptAccordion, {
    reusePinia: true,
    mount: { props: { serverId } }
  });
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("McpPromptAccordion", () => {
  it("renders collapsed by default with prompt count", async () => {
    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`);
    expect(toggle.exists()).toBe(true);
    expect(toggle.attributes("aria-expanded")).toBe("false");
    expect(wrapper.find(`[data-test="mcp-prompts-list-${SERVER_ID}"]`).exists()).toBe(false);
  });

  it("expands on toggle click and calls fetchPrompts", async () => {
    const mcp = useMcpStore();
    const fetchSpy = vi.spyOn(mcp, "fetchPrompts").mockResolvedValue(undefined);

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`);
    await toggle.trigger("click");
    await flushPromises();

    expect(toggle.attributes("aria-expanded")).toBe("true");
    expect(fetchSpy).toHaveBeenCalledWith(SERVER_ID);
    expect(wrapper.find(`[data-test="mcp-prompts-list-${SERVER_ID}"]`).exists()).toBe(true);
  });

  it("collapses on second toggle click", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchPrompts").mockResolvedValue(undefined);

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`);
    await toggle.trigger("click");
    await flushPromises();
    expect(toggle.attributes("aria-expanded")).toBe("true");

    await toggle.trigger("click");
    await flushPromises();
    expect(toggle.attributes("aria-expanded")).toBe("false");
    expect(wrapper.find(`[data-test="mcp-prompts-list-${SERVER_ID}"]`).exists()).toBe(false);
  });

  it("shows loading text when prompts are loading", async () => {
    const mcp = useMcpStore();
    mcp.loadingPrompts = new Set([SERVER_ID]);

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`);
    expect(toggle.text()).toContain("Loading");
  });

  it("shows empty state when expanded with no prompts", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchPrompts").mockResolvedValue(undefined);
    mcp.serverPrompts = { [SERVER_ID]: [] };

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`);
    await toggle.trigger("click");
    await flushPromises();

    expect(wrapper.find(`[data-test="mcp-prompts-empty-${SERVER_ID}"]`).exists()).toBe(true);
  });

  it("shows error state when promptsError is set", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchPrompts").mockResolvedValue(undefined);
    mcp.promptsError = { [SERVER_ID]: "Connection refused" };

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`);
    await toggle.trigger("click");
    await flushPromises();

    const errorEl = wrapper.find(`[data-test="mcp-prompts-error-${SERVER_ID}"]`);
    expect(errorEl.exists()).toBe(true);
    expect(errorEl.text()).toContain("Connection refused");
  });

  it("renders prompt list items with name, argument count, and description", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchPrompts").mockResolvedValue(undefined);
    mcp.serverPrompts = {
      [SERVER_ID]: [
        makePrompt({ name: "summarize", description: "Summarize text", argument_count: 1 }),
        makePrompt({ name: "translate", description: "Translate text", argument_count: 3 })
      ]
    };

    const { wrapper } = mountAccordion();
    await flushPromises();

    await wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`).trigger("click");
    await flushPromises();

    const item1 = wrapper.find(`[data-test="mcp-prompt-${SERVER_ID}-summarize"]`);
    expect(item1.exists()).toBe(true);
    expect(item1.find(".prompt-name").text()).toBe("summarize");
    expect(item1.text()).toContain("1");
    expect(item1.find(".prompt-desc").text()).toBe("Summarize text");

    const item2 = wrapper.find(`[data-test="mcp-prompt-${SERVER_ID}-translate"]`);
    expect(item2.exists()).toBe(true);
    expect(item2.find(".prompt-name").text()).toBe("translate");
    expect(item2.text()).toContain("3");
  });

  it("renders prompt without description when description is null", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchPrompts").mockResolvedValue(undefined);
    mcp.serverPrompts = {
      [SERVER_ID]: [makePrompt({ name: "no-desc", description: null, argument_count: 0 })]
    };

    const { wrapper } = mountAccordion();
    await flushPromises();

    await wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`).trigger("click");
    await flushPromises();

    const item = wrapper.find(`[data-test="mcp-prompt-${SERVER_ID}-no-desc"]`);
    expect(item.exists()).toBe(true);
    expect(item.find(".prompt-desc").exists()).toBe(false);
  });

  it("uses correct toggle icons for collapsed and expanded states", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchPrompts").mockResolvedValue(undefined);

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`);
    expect(toggle.find(".toggle-icon").text()).toBe("▶");

    await toggle.trigger("click");
    await flushPromises();
    expect(toggle.find(".toggle-icon").text()).toBe("▼");
  });

  it("does not call fetchPrompts when collapsing", async () => {
    const mcp = useMcpStore();
    const fetchSpy = vi.spyOn(mcp, "fetchPrompts").mockResolvedValue(undefined);

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`);
    await toggle.trigger("click");
    expect(fetchSpy).toHaveBeenCalledTimes(1);

    await toggle.trigger("click");
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it("does not show empty state while loading", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchPrompts").mockResolvedValue(undefined);
    mcp.serverPrompts = { [SERVER_ID]: [] };
    mcp.loadingPrompts = new Set([SERVER_ID]);

    const { wrapper } = mountAccordion();
    await flushPromises();

    await wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`).trigger("click");
    await flushPromises();

    expect(wrapper.find(`[data-test="mcp-prompts-empty-${SERVER_ID}"]`).exists()).toBe(false);
  });

  it("error state takes precedence over empty state", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchPrompts").mockResolvedValue(undefined);
    mcp.serverPrompts = { [SERVER_ID]: [] };
    mcp.promptsError = { [SERVER_ID]: "Server unreachable" };

    const { wrapper } = mountAccordion();
    await flushPromises();

    await wrapper.find(`[data-test="mcp-prompts-toggle-${SERVER_ID}"]`).trigger("click");
    await flushPromises();

    expect(wrapper.find(`[data-test="mcp-prompts-error-${SERVER_ID}"]`).exists()).toBe(true);
    expect(wrapper.find(`[data-test="mcp-prompts-empty-${SERVER_ID}"]`).exists()).toBe(false);
  });
});
