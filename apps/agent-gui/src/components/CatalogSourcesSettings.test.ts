import { describe, it, expect, beforeEach, vi } from "vitest";
import { mount, flushPromises } from "@vue/test-utils";
import CatalogSourcesSettings from "./CatalogSourcesSettings.vue";
import { resetCatalogState } from "../stores/catalog";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));
vi.mock("../composables/useNotifications", () => ({
  addNotification: vi.fn(),
  dismissNotification: vi.fn(),
  notifications: []
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

const sampleSource = {
  id: "smithery",
  display_name: "Smithery",
  kind: "smithery",
  url: "https://registry.smithery.ai",
  api_key_env: null,
  priority: 50,
  default_trust: "community",
  enabled: true,
  cache_ttl_seconds: null,
  last_error: null
};

describe("CatalogSourcesSettings.vue", () => {
  beforeEach(() => {
    resetCatalogState();
    vi.clearAllMocks();
  });

  it("renders empty state when no sources configured", async () => {
    mockedInvoke.mockResolvedValueOnce([] as never);
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    expect(wrapper.text()).toContain("No remote catalog sources");
  });

  it("renders configured sources", async () => {
    mockedInvoke.mockResolvedValueOnce([sampleSource] as never);
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    expect(wrapper.text()).toContain("Smithery");
    expect(wrapper.text()).toContain("registry.smithery.ai");
  });

  it("validates url before calling addSource", async () => {
    mockedInvoke.mockResolvedValueOnce([] as never);
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    await wrapper.find('[data-test="add-source-toggle"]').trigger("click");
    await wrapper.find('[data-test="src-id"]').setValue("x");
    await wrapper.find('[data-test="src-name"]').setValue("X");
    await wrapper.find('[data-test="src-url"]').setValue("not-a-url");
    await wrapper.find('[data-test="src-save"]').trigger("click");
    await flushPromises();
    expect(wrapper.text().toLowerCase()).toContain("url must start with http");
    // Only the initial list_catalog_sources call should have happened.
    const addCalls = mockedInvoke.mock.calls.filter(
      (c) => c[0] === "add_catalog_source"
    );
    expect(addCalls).toHaveLength(0);
  });

  it("calls addSource with the form payload on save", async () => {
    mockedInvoke
      .mockResolvedValueOnce([] as never) // initial load
      .mockResolvedValueOnce(undefined as never) // add
      .mockResolvedValueOnce([] as never); // reload
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    await wrapper.find('[data-test="add-source-toggle"]').trigger("click");
    await wrapper.find('[data-test="src-id"]').setValue("x");
    await wrapper.find('[data-test="src-name"]').setValue("X");
    await wrapper.find('[data-test="src-url"]').setValue("https://x/c.json");
    await wrapper.find('[data-test="src-save"]').trigger("click");
    await flushPromises();
    const addCall = mockedInvoke.mock.calls.find(
      (c) => c[0] === "add_catalog_source"
    );
    expect(addCall).toBeDefined();
    expect(addCall![1]).toMatchObject({
      request: { id: "x", url: "https://x/c.json" }
    });
  });

  it("removes a source via the remove button", async () => {
    mockedInvoke.mockResolvedValueOnce([
      { ...sampleSource, id: "x", display_name: "X", kind: "kairox_json" }
    ] as never);
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    mockedInvoke
      .mockResolvedValueOnce(undefined as never)
      .mockResolvedValueOnce([] as never);
    await wrapper.find('[data-test="src-remove-x"]').trigger("click");
    await flushPromises();
    const removeCall = mockedInvoke.mock.calls.find(
      (c) => c[0] === "remove_catalog_source"
    );
    expect(removeCall).toBeDefined();
    expect(removeCall![1]).toMatchObject({ id: "x" });
  });
});
