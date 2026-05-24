import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { flushPromises } from "@vue/test-utils";
import CatalogSourcesSettings from "./CatalogSourcesSettings.vue";
import catalogSourcesSettingsSource from "./CatalogSourcesSettings.vue?raw";
import { mountWithPlugins } from "@/test-utils/mount";
import { expectSourceMigration } from "@/test-utils/sourceGuards";
import { useCatalogStore } from "@/stores/catalog";

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

const internalSource = {
  ...sampleSource,
  id: "team-registry",
  display_name: "Team Registry",
  url: "https://registry.internal.example",
  enabled: false
};

function visibleSourceRowIds(wrapper: ReturnType<typeof mount>): string[] {
  return wrapper
    .findAll('[data-test^="catalog-source-row-"]')
    .map((row) => row.attributes("data-test")?.replace("catalog-source-row-", "") ?? "");
}

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
    expect(wrapper.find('[data-test="catalog-sources-list"]').classes()).toContain(
      "settings-card-list"
    );
    expect(wrapper.find('[data-test="catalog-source-row-mcp-registry"]').classes()).toContain(
      "settings-card-item"
    );
    expect(wrapper.find(".settings-card-item__actions.kx-action-group").exists()).toBe(true);
  });

  it("filters configured sources by searchable source fields", async () => {
    mockedInvoke.mockResolvedValueOnce([sampleSource, internalSource] as never);
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();
    const catalog = useCatalogStore();
    catalog.handleSourceFailed("team-registry", "Timeout contacting registry");
    await flushPromises();

    expect(wrapper.find('[data-test="catalog-source-search-input"]').exists()).toBe(true);

    await wrapper.find('[data-test="catalog-source-search-input"]').setValue("internal");

    expect(wrapper.find('[data-test="catalog-source-row-team-registry"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="catalog-source-row-mcp-registry"]').exists()).toBe(false);

    await wrapper.find('[data-test="catalog-source-search-input"]').setValue("disabled");

    expect(wrapper.find('[data-test="catalog-source-row-team-registry"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="catalog-source-row-mcp-registry"]').exists()).toBe(false);

    await wrapper.find('[data-test="catalog-source-search-input"]').setValue("timeout");

    expect(wrapper.find('[data-test="catalog-source-row-team-registry"]').exists()).toBe(true);
    expect(wrapper.find('[data-test="catalog-source-row-mcp-registry"]').exists()).toBe(false);
  });

  it("shows a filtered empty state when no catalog sources match search", async () => {
    mockedInvoke.mockResolvedValueOnce([sampleSource, internalSource] as never);
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();

    await wrapper.find('[data-test="catalog-source-search-input"]').setValue("does-not-exist");

    expect(wrapper.find('[data-test="catalog-sources-list"]').exists()).toBe(false);
    const empty = wrapper.find('[data-test="catalog-sources-filter-empty"]');
    expect(empty.exists()).toBe(true);
    expect(empty.text()).toContain("No remote catalog sources match your search.");
  });

  it("sorts filtered catalog sources without mutating the catalog source order", async () => {
    const zetaSource = {
      ...sampleSource,
      id: "zeta-source",
      display_name: "Zeta Team Source",
      priority: 30
    };
    const alphaSource = {
      ...sampleSource,
      id: "alpha-source",
      display_name: "Alpha Team Source",
      priority: 10
    };
    const hiddenSource = {
      ...sampleSource,
      id: "hidden-source",
      display_name: "Hidden Source",
      priority: 20
    };
    mockedInvoke.mockResolvedValueOnce([zetaSource, alphaSource, hiddenSource] as never);
    const wrapper = mount(CatalogSourcesSettings);
    await flushPromises();

    const sortSelect = wrapper.find('[data-test="catalog-source-sort-select"]');
    expect(sortSelect.exists()).toBe(true);
    expect(sortSelect.attributes("aria-label")).toBe("Catalog source sort");

    await wrapper.find('[data-test="catalog-source-search-input"]').setValue("team");
    expect(visibleSourceRowIds(wrapper)).toEqual(["zeta-source", "alpha-source"]);

    await sortSelect.setValue("name");
    expect(visibleSourceRowIds(wrapper)).toEqual(["alpha-source", "zeta-source"]);

    const catalog = useCatalogStore();
    expect(catalog.sources.map((source) => source.id)).toEqual([
      "zeta-source",
      "alpha-source",
      "hidden-source"
    ]);
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
    expectSourceMigration(catalogSourcesSettingsSource, {
      required: [
        "SettingsFilterBar",
        "SettingsCardList",
        "SettingsCardItem",
        "SettingsStatusTag",
        "<template #actions>",
        "KxFormActions",
        "KxInput",
        "KxSelect"
      ],
      forbidden: [
        "tag-info",
        'class="src-actions"',
        ".src-actions {",
        "kx-form-control",
        'class="input"',
        ".input {",
        ".form-actions {"
      ]
    });
  });

  it("does not keep catalog source aria, option, or form helper copy inline", () => {
    expectSourceMigration(catalogSourcesSettingsSource, {
      forbidden: [
        'aria-label="Catalog sources"',
        'label="id"',
        'label: "MCP Registry"',
        'placeholder="https://registry.example/catalog.json"',
        "Optional environment variable used for authenticated catalog requests."
      ]
    });
  });
});
