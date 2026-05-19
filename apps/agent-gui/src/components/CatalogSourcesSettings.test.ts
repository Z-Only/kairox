import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { flushPromises } from "@vue/test-utils";
import CatalogSourcesSettings from "./CatalogSourcesSettings.vue";
import catalogSourcesSettingsSource from "./CatalogSourcesSettings.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";

// `CatalogSourcesSettings.vue` calls `useI18n()`, which requires a Vue plugin
// install — bare `mount()` throws "Need to install with `app.use` function".
// `mountWithPlugins` installs i18n + router; `reusePinia: true` is critical
// because each test's `beforeEach` already calls `setActivePinia(createPinia())`,
// and we don't want `mountWithPlugins` to wipe those store mutations.
//
// Passing the extended-options shape returns `{ wrapper, router }`; we
// unwrap `.wrapper` so call-sites stay drop-in compatible with the prior
// `mount(...)` usage.
const mount = (comp: typeof CatalogSourcesSettings) =>
  mountWithPlugins(comp, { reusePinia: true }).wrapper;

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

const sampleSource = {
  id: "mcp-registry",
  display_name: "Model Context Protocol Servers",
  kind: "mcp_registry",
  url: "https://registry.modelcontextprotocol.io",
  api_key_env: null,
  priority: 50,
  default_trust: "community",
  enabled: true,
  cache_ttl_seconds: null,
  last_error: null
};

describe("CatalogSourcesSettings.vue", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
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
    expect(wrapper.text()).toContain("Model Context Protocol Servers");
    expect(wrapper.text()).toContain("registry.modelcontextprotocol.io");
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
    const addCalls = mockedInvoke.mock.calls.filter((c) => c[0] === "add_catalog_source");
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
    const addCall = mockedInvoke.mock.calls.find((c) => c[0] === "add_catalog_source");
    expect(addCall).toBeDefined();
    expect(addCall![1]).toMatchObject({
      request: { id: "x", url: "https://x/c.json" }
    });
  });

  it("removes a source via the remove button", async () => {
    mockedInvoke.mockResolvedValueOnce([
      { ...sampleSource, id: "x", display_name: "X", kind: "mcp_registry" }
    ] as never);
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    mockedInvoke.mockResolvedValueOnce(undefined as never).mockResolvedValueOnce([] as never);
    await wrapper.find('[data-test="src-remove-x"]').trigger("click");
    await flushPromises();
    const removeCall = mockedInvoke.mock.calls.find((c) => c[0] === "remove_catalog_source");
    expect(removeCall).toBeDefined();
    expect(removeCall![1]).toMatchObject({ id: "x" });
  });

  it("uses shared form controls and action rows in the add-source form", () => {
    expect(catalogSourcesSettingsSource).toContain("KxFormActions");
    expect(catalogSourcesSettingsSource).toContain("KxInput");
    expect(catalogSourcesSettingsSource).toContain("KxSelect");
    expect(catalogSourcesSettingsSource).not.toContain("kx-form-control");
    expect(catalogSourcesSettingsSource).not.toContain('class="input"');
    expect(catalogSourcesSettingsSource).not.toContain(".input {");
    expect(catalogSourcesSettingsSource).not.toContain(".form-actions {");
  });
});
