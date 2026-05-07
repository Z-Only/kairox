import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { createUiStoreMock } from "@/test-utils/uiMock";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

const pushNotificationSpy = vi.fn();
vi.mock("@/stores/ui", () => ({
  useUiStore: () => createUiStoreMock({ pushNotification: pushNotificationSpy })
}));

import { invoke } from "@tauri-apps/api/core";
import { useCatalogStore } from "@/stores/catalog";

const mockedInvoke = vi.mocked(invoke);

const fixtureEntry = (over: Partial<Record<string, unknown>> = {}) => ({
  id: "filesystem",
  source: "builtin",
  display_name: "Filesystem",
  summary: "s",
  description: "d",
  categories: ["filesystem"],
  tags: [],
  author: null,
  homepage: null,
  version: null,
  trust: "verified",
  icon: "📁",
  install_spec_json: "{}",
  requirements_json: "[]",
  default_env_json: "[]",
  ...over
});

describe("catalog store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    pushNotificationSpy.mockClear();
  });

  it("loads entries via list_catalog", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockResolvedValueOnce([fixtureEntry()] as never);
    await catalog.fetchCatalog();
    expect(mockedInvoke).toHaveBeenCalledWith("list_catalog", {
      query: expect.any(Object)
    });
    expect(catalog.entries.length).toBe(1);
    expect(catalog.entries[0].id).toBe("filesystem");
  });

  it("install dispatches install_catalog_entry and stores outcome", async () => {
    const catalog = useCatalogStore();
    mockedInvoke
      .mockResolvedValueOnce({
        kind: "installed",
        server_id: "filesystem",
        started: true,
        missing_runtimes: [],
        missing_env_keys: []
      } as never)
      .mockResolvedValueOnce([] as never); // refreshInstalled

    const outcome = await catalog.installEntry({
      catalog_id: "filesystem",
      source: "builtin",
      server_id_override: null,
      env_overrides: { WORKSPACE_PATH: "/tmp" },
      trust_grant: true,
      auto_start: true
    });

    expect(outcome?.kind).toBe("installed");
    expect(catalog.installState["filesystem"]).toEqual({
      kind: "installed",
      server_id: "filesystem",
      started: true,
      missing_runtimes: [],
      missing_env_keys: []
    });
  });

  it("filters by keyword + trust client-side", () => {
    const catalog = useCatalogStore();
    catalog.entries = [
      fixtureEntry({
        id: "a",
        display_name: "Alpha",
        summary: "x",
        tags: ["alpha"],
        trust: "verified"
      }),
      fixtureEntry({
        id: "b",
        display_name: "Beta",
        summary: "y",
        tags: ["beta"],
        trust: "community"
      })
    ];
    catalog.filters.keyword = "alpha";
    catalog.filters.trustMin = "verified";
    expect(catalog.filteredEntries.map((e) => e.id)).toEqual(["a"]);
  });

  it("uninstall removes from installState and refreshes installed", async () => {
    const catalog = useCatalogStore();
    catalog.installState["filesystem"] = {
      kind: "installed",
      server_id: "filesystem",
      started: true,
      missing_runtimes: [],
      missing_env_keys: []
    };
    mockedInvoke
      .mockResolvedValueOnce(undefined as never) // uninstall_catalog_entry
      .mockResolvedValueOnce([] as never); // list_installed_entries

    await catalog.uninstallEntry("filesystem");

    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "uninstall_catalog_entry", {
      serverId: "filesystem"
    });
    expect(catalog.installState["filesystem"]).toBeUndefined();
  });

  it("refreshCatalogSource calls refresh_catalog then re-fetches", async () => {
    const catalog = useCatalogStore();
    mockedInvoke
      .mockResolvedValueOnce(undefined as never) // refresh_catalog
      .mockResolvedValueOnce([] as never); // list_catalog

    await catalog.refreshCatalogSource("builtin");

    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "refresh_catalog", {
      source: "builtin"
    });
    expect(mockedInvoke).toHaveBeenNthCalledWith(2, "list_catalog", {
      query: expect.any(Object)
    });
  });

  it("fetchInstalled populates installed list", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockResolvedValueOnce([
      {
        server_id: "filesystem",
        catalog_id: "filesystem",
        source: "builtin",
        display_name: "Filesystem",
        installed_at: "2026-05-06T00:00:00Z",
        running: true
      }
    ] as never);
    await catalog.fetchInstalled();
    expect(catalog.installed.length).toBe(1);
    expect(catalog.installed[0].server_id).toBe("filesystem");
  });
});

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

describe("catalog store — Phase 2 sources", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    pushNotificationSpy.mockClear();
  });

  it("fetchSources loads sources via list_catalog_sources", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockResolvedValueOnce([sampleSource] as never);
    await catalog.fetchSources();
    expect(mockedInvoke).toHaveBeenCalledWith("list_catalog_sources");
    expect(catalog.sources).toHaveLength(1);
    expect(catalog.sources[0].id).toBe("smithery");
  });

  it("addSource calls add_catalog_source then re-fetches", async () => {
    const catalog = useCatalogStore();
    mockedInvoke
      .mockResolvedValueOnce(undefined as never)
      .mockResolvedValueOnce([sampleSource] as never);
    await catalog.addSource({
      id: "smithery",
      display_name: "Smithery",
      kind: "smithery",
      url: "https://registry.smithery.ai",
      api_key_env: null,
      priority: 50,
      default_trust: "community",
      enabled: true,
      cache_ttl_seconds: null
    });
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "add_catalog_source", {
      request: expect.objectContaining({ id: "smithery" })
    });
    expect(mockedInvoke).toHaveBeenNthCalledWith(2, "list_catalog_sources");
    expect(catalog.sources).toHaveLength(1);
  });

  it("removeSource calls remove_catalog_source then re-fetches", async () => {
    const catalog = useCatalogStore();
    catalog.sources = [sampleSource];
    mockedInvoke.mockResolvedValueOnce(undefined as never).mockResolvedValueOnce([] as never);
    await catalog.removeSource("smithery");
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "remove_catalog_source", {
      id: "smithery"
    });
    expect(catalog.sources).toHaveLength(0);
  });

  it("setSourceEnabled toggles a source and re-fetches", async () => {
    const catalog = useCatalogStore();
    catalog.sources = [sampleSource];
    mockedInvoke
      .mockResolvedValueOnce(undefined as never)
      .mockResolvedValueOnce([{ ...sampleSource, enabled: false }] as never);
    await catalog.setSourceEnabled("smithery", false);
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "set_catalog_source_enabled", {
      id: "smithery",
      enabled: false
    });
    expect(catalog.sources[0].enabled).toBe(false);
  });

  it("handleSourceFailed records sourceFailures keyed by source id", () => {
    const catalog = useCatalogStore();
    catalog.handleSourceFailed("smithery", "timeout");
    expect(catalog.sourceFailures.smithery).toBe("timeout");
  });
});
