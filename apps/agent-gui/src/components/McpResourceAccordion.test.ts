import { describe, it, expect, vi, beforeEach } from "vitest";
import { flushPromises } from "@vue/test-utils";
import { setActivePinia, createPinia } from "pinia";
import { mountWithPlugins } from "@/test-utils/mount";
import type { McpResourceDefResponse, McpContentBlockResponse } from "@/generated/commands";
import { useMcpStore } from "@/stores/mcp";
import McpResourceAccordion from "./McpResourceAccordion.vue";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

vi.mock("@/generated/commands", () => ({
  commands: {
    listMcpResources: vi.fn(),
    listMcpPrompts: vi.fn(),
    readMcpResource: vi.fn(),
    listMcpServerSettings: vi.fn(),
    getEffectiveMcpServers: vi.fn(),
    checkMcpHealth: vi.fn(),
    getMcpToolStates: vi.fn(),
    testMcpConnectivity: vi.fn()
  }
}));

const SERVER_ID = "test-mcp-server";

function makeResource(overrides: Partial<McpResourceDefResponse> = {}): McpResourceDefResponse {
  return {
    uri: "file:///docs/readme.md",
    name: "readme.md",
    description: "Project README",
    mime_type: "text/markdown",
    ...overrides
  };
}

function mountAccordion(serverId = SERVER_ID) {
  return mountWithPlugins(McpResourceAccordion, {
    reusePinia: true,
    mount: { props: { serverId } }
  });
}

beforeEach(() => {
  setActivePinia(createPinia());
  vi.clearAllMocks();
});

describe("McpResourceAccordion", () => {
  it("renders collapsed by default with resource count", async () => {
    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`);
    expect(toggle.exists()).toBe(true);
    expect(toggle.attributes("aria-expanded")).toBe("false");
    expect(wrapper.find(`[data-test="mcp-resources-list-${SERVER_ID}"]`).exists()).toBe(false);
  });

  it("expands on toggle click and calls fetchResources", async () => {
    const mcp = useMcpStore();
    const fetchSpy = vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`);
    await toggle.trigger("click");
    await flushPromises();

    expect(toggle.attributes("aria-expanded")).toBe("true");
    expect(fetchSpy).toHaveBeenCalledWith(SERVER_ID);
    expect(wrapper.find(`[data-test="mcp-resources-list-${SERVER_ID}"]`).exists()).toBe(true);
  });

  it("collapses on second toggle click", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`);
    await toggle.trigger("click");
    await flushPromises();
    expect(toggle.attributes("aria-expanded")).toBe("true");

    await toggle.trigger("click");
    await flushPromises();
    expect(toggle.attributes("aria-expanded")).toBe("false");
    expect(wrapper.find(`[data-test="mcp-resources-list-${SERVER_ID}"]`).exists()).toBe(false);
  });

  it("shows loading text when resources are loading", async () => {
    const mcp = useMcpStore();
    mcp.loadingResources = new Set([SERVER_ID]);

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`);
    expect(toggle.text()).toContain("Loading");
  });

  it("shows empty state when expanded with no resources", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);
    mcp.serverResources = { [SERVER_ID]: [] };

    const { wrapper } = mountAccordion();
    await flushPromises();

    await wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`).trigger("click");
    await flushPromises();

    expect(wrapper.find(`[data-test="mcp-resources-empty-${SERVER_ID}"]`).exists()).toBe(true);
  });

  it("shows error state when resourcesError is set", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);
    mcp.resourcesError = { [SERVER_ID]: "Connection refused" };

    const { wrapper } = mountAccordion();
    await flushPromises();

    await wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`).trigger("click");
    await flushPromises();

    const errorEl = wrapper.find(`[data-test="mcp-resources-error-${SERVER_ID}"]`);
    expect(errorEl.exists()).toBe(true);
    expect(errorEl.text()).toContain("Connection refused");
  });

  it("renders resource list items with name, uri, and mime type", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);
    mcp.serverResources = {
      [SERVER_ID]: [
        makeResource({
          uri: "file:///a.md",
          name: "a.md",
          mime_type: "text/markdown"
        }),
        makeResource({
          uri: "file:///b.json",
          name: "b.json",
          mime_type: "application/json"
        })
      ]
    };

    const { wrapper } = mountAccordion();
    await flushPromises();

    await wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`).trigger("click");
    await flushPromises();

    const item1 = wrapper.find(`[data-test="mcp-resource-${SERVER_ID}-a.md"]`);
    expect(item1.exists()).toBe(true);
    expect(item1.find(".resource-name").text()).toBe("a.md");
    expect(item1.find(".resource-uri").text()).toBe("file:///a.md");
    expect(item1.text()).toContain("text/markdown");

    const item2 = wrapper.find(`[data-test="mcp-resource-${SERVER_ID}-b.json"]`);
    expect(item2.exists()).toBe(true);
    expect(item2.find(".resource-name").text()).toBe("b.json");
  });

  it("does not render mime tag when mime_type is null", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);
    mcp.serverResources = {
      [SERVER_ID]: [makeResource({ name: "no-mime", uri: "file:///x", mime_type: null })]
    };

    const { wrapper } = mountAccordion();
    await flushPromises();

    await wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`).trigger("click");
    await flushPromises();

    const item = wrapper.find(`[data-test="mcp-resource-${SERVER_ID}-no-mime"]`);
    expect(item.exists()).toBe(true);
    expect(item.find(".mime-token").exists()).toBe(false);
  });

  it("uses correct toggle icons for collapsed and expanded states", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`);
    expect(toggle.find(".toggle-icon").text()).toBe("▶");

    await toggle.trigger("click");
    await flushPromises();
    expect(toggle.find(".toggle-icon").text()).toBe("▼");
  });

  it("does not call fetchResources when collapsing", async () => {
    const mcp = useMcpStore();
    const fetchSpy = vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);

    const { wrapper } = mountAccordion();
    await flushPromises();

    const toggle = wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`);
    await toggle.trigger("click");
    expect(fetchSpy).toHaveBeenCalledTimes(1);

    await toggle.trigger("click");
    expect(fetchSpy).toHaveBeenCalledTimes(1);
  });

  it("does not show empty state while loading", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);
    mcp.serverResources = { [SERVER_ID]: [] };
    mcp.loadingResources = new Set([SERVER_ID]);

    const { wrapper } = mountAccordion();
    await flushPromises();

    await wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`).trigger("click");
    await flushPromises();

    expect(wrapper.find(`[data-test="mcp-resources-empty-${SERVER_ID}"]`).exists()).toBe(false);
  });

  it("error state takes precedence over empty state", async () => {
    const mcp = useMcpStore();
    vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);
    mcp.serverResources = { [SERVER_ID]: [] };
    mcp.resourcesError = { [SERVER_ID]: "Server unreachable" };

    const { wrapper } = mountAccordion();
    await flushPromises();

    await wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`).trigger("click");
    await flushPromises();

    expect(wrapper.find(`[data-test="mcp-resources-error-${SERVER_ID}"]`).exists()).toBe(true);
    expect(wrapper.find(`[data-test="mcp-resources-empty-${SERVER_ID}"]`).exists()).toBe(false);
  });

  describe("resource content expansion", () => {
    it("expands a resource item and shows text content blocks", async () => {
      const mcp = useMcpStore();
      vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);
      vi.spyOn(mcp, "readResource").mockResolvedValue([
        { type: "text", text: "Hello world" } as McpContentBlockResponse
      ]);

      const resource = makeResource({ uri: "file:///doc.md", name: "doc.md" });
      mcp.serverResources = { [SERVER_ID]: [resource] };

      const { wrapper } = mountAccordion();
      await flushPromises();

      // Expand the accordion first
      await wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`).trigger("click");
      await flushPromises();

      // Click the resource item
      const item = wrapper.find(`[data-test="mcp-resource-${SERVER_ID}-doc.md"]`);
      expect(item.exists()).toBe(true);

      await item.trigger("click");
      await flushPromises();

      // Set the expanded state and content cache as the component expects
      mcp.expandedResourceUri = { [SERVER_ID]: "file:///doc.md" };
      mcp.resourceContentCache = {
        [`${SERVER_ID}:file:///doc.md`]: [{ type: "text", text: "Hello world" }]
      };
      await flushPromises();

      const content = wrapper.find(`[data-test="mcp-resource-content-${SERVER_ID}-doc.md"]`);
      expect(content.exists()).toBe(true);
      expect(content.find(".content-block__text").text()).toBe("Hello world");
    });

    it("renders image content blocks", async () => {
      const mcp = useMcpStore();
      vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);

      const resource = makeResource({ uri: "file:///img.png", name: "img.png" });
      mcp.serverResources = { [SERVER_ID]: [resource] };
      mcp.expandedResourceUri = { [SERVER_ID]: "file:///img.png" };
      mcp.resourceContentCache = {
        [`${SERVER_ID}:file:///img.png`]: [
          { type: "image", data: "base64data", mime_type: "image/png" }
        ]
      };

      const { wrapper } = mountAccordion();
      await flushPromises();

      await wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`).trigger("click");
      await flushPromises();

      const content = wrapper.find(`[data-test="mcp-resource-content-${SERVER_ID}-img.png"]`);
      expect(content.exists()).toBe(true);
      const img = content.find(".content-block__image");
      expect(img.exists()).toBe(true);
      expect(img.attributes("src")).toBe("data:image/png;base64,base64data");
      expect(img.attributes("alt")).toBe("img.png");
    });

    it("renders resource link content blocks", async () => {
      const mcp = useMcpStore();
      vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);

      const resource = makeResource({ uri: "file:///link", name: "link-res" });
      mcp.serverResources = { [SERVER_ID]: [resource] };
      mcp.expandedResourceUri = { [SERVER_ID]: "file:///link" };
      mcp.resourceContentCache = {
        [`${SERVER_ID}:file:///link`]: [
          { type: "resource", uri: "https://example.com", name: "Example", mime_type: null }
        ]
      };

      const { wrapper } = mountAccordion();
      await flushPromises();

      await wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`).trigger("click");
      await flushPromises();

      const content = wrapper.find(`[data-test="mcp-resource-content-${SERVER_ID}-link-res"]`);
      expect(content.exists()).toBe(true);
      const link = content.find(".content-block__link");
      expect(link.exists()).toBe(true);
      expect(link.attributes("href")).toBe("https://example.com");
      expect(link.text()).toBe("Example");
    });

    it("shows resource uri as link text when name is empty", async () => {
      const mcp = useMcpStore();
      vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);

      const resource = makeResource({ uri: "file:///fallback", name: "fallback-res" });
      mcp.serverResources = { [SERVER_ID]: [resource] };
      mcp.expandedResourceUri = { [SERVER_ID]: "file:///fallback" };
      mcp.resourceContentCache = {
        [`${SERVER_ID}:file:///fallback`]: [
          { type: "resource", uri: "https://example.com/path", name: "", mime_type: null }
        ]
      };

      const { wrapper } = mountAccordion();
      await flushPromises();

      await wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`).trigger("click");
      await flushPromises();

      const link = wrapper.find(".content-block__link");
      expect(link.text()).toBe("https://example.com/path");
    });

    it("shows correct toggle icon on expanded resource items", async () => {
      const mcp = useMcpStore();
      vi.spyOn(mcp, "fetchResources").mockResolvedValue(undefined);

      const resource = makeResource({ uri: "file:///a", name: "a" });
      mcp.serverResources = { [SERVER_ID]: [resource] };

      const { wrapper } = mountAccordion();
      await flushPromises();

      await wrapper.find(`[data-test="mcp-resources-toggle-${SERVER_ID}"]`).trigger("click");
      await flushPromises();

      const item = wrapper.find(`[data-test="mcp-resource-${SERVER_ID}-a"]`);
      // Collapsed resource item shows right arrow
      expect(item.find(".toggle-icon").text()).toBe("▶");

      // Expand the resource
      mcp.expandedResourceUri = { [SERVER_ID]: "file:///a" };
      await flushPromises();

      expect(item.find(".toggle-icon").text()).toBe("▼");
      expect(item.attributes("aria-expanded")).toBe("true");
    });
  });
});
