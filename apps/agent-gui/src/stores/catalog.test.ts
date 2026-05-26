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
    expect(catalog.sources[0].id).toBe("mcp-registry");
  });

  it("addSource calls add_catalog_source then re-fetches", async () => {
    const catalog = useCatalogStore();
    mockedInvoke
      .mockResolvedValueOnce(undefined as never)
      .mockResolvedValueOnce([sampleSource] as never);
    await catalog.addSource({
      id: "mcp-registry",
      display_name: "Model Context Protocol Servers",
      kind: "mcp_registry",
      url: "https://registry.modelcontextprotocol.io",
      api_key_env: null,
      priority: 50,
      default_trust: "community",
      enabled: true,
      cache_ttl_seconds: null
    });
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "add_catalog_source", {
      request: expect.objectContaining({ id: "mcp-registry" })
    });
    expect(mockedInvoke).toHaveBeenNthCalledWith(2, "list_catalog_sources");
    expect(catalog.sources).toHaveLength(1);
  });

  it("removeSource calls remove_catalog_source then re-fetches", async () => {
    const catalog = useCatalogStore();
    catalog.sources = [sampleSource];
    mockedInvoke.mockResolvedValueOnce(undefined as never).mockResolvedValueOnce([] as never);
    await catalog.removeSource("mcp-registry");
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "remove_catalog_source", {
      id: "mcp-registry"
    });
    expect(catalog.sources).toHaveLength(0);
  });

  it("setSourceEnabled toggles a source and re-fetches", async () => {
    const catalog = useCatalogStore();
    catalog.sources = [sampleSource];
    mockedInvoke
      .mockResolvedValueOnce(undefined as never)
      .mockResolvedValueOnce([{ ...sampleSource, enabled: false }] as never);
    await catalog.setSourceEnabled("mcp-registry", false);
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "set_catalog_source_enabled", {
      id: "mcp-registry",
      enabled: false
    });
    expect(catalog.sources[0].enabled).toBe(false);
  });

  it("handleSourceFailed records sourceFailures keyed by source id", () => {
    const catalog = useCatalogStore();
    catalog.handleSourceFailed("mcp-registry", "timeout");
    expect(catalog.sourceFailures["mcp-registry"]).toBe("timeout");
  });

  it("setSourceEnabled surfaces a UI error notification when invoke rejects", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockRejectedValueOnce(new Error("backend offline"));
    await catalog.setSourceEnabled("mcp-registry", false);
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Failed to toggle source mcp-registry")
    );
  });

  it("mergeSourceResults clears prior failure for the source it merges", () => {
    const catalog = useCatalogStore();
    catalog.handleSourceFailed("registry-a", "earlier failure");
    catalog.mergeSourceResults("registry-a", []);
    expect(catalog.sourceFailures["registry-a"]).toBeUndefined();
  });

  it("mergeSourceResults sorts by trust desc, source asc, then display_name asc", () => {
    const catalog = useCatalogStore();
    catalog.entries = [
      fixtureEntry({
        id: "alpha",
        source: "registry-b",
        display_name: "Alpha",
        trust: "community"
      })
    ] as never;
    catalog.mergeSourceResults("registry-a", [
      fixtureEntry({
        id: "beta",
        source: "registry-a",
        display_name: "Beta",
        trust: "verified"
      }),
      fixtureEntry({
        id: "gamma",
        source: "registry-a",
        display_name: "Gamma",
        trust: "community"
      }),
      fixtureEntry({
        id: "delta",
        source: "registry-a",
        display_name: "Delta",
        trust: "community"
      })
    ] as never);

    const order = catalog.entries.map((entry) => entry.display_name);
    // verified first; within community: registry-a sorts before registry-b;
    // within registry-a community: Delta sorts before Gamma alphabetically.
    expect(order).toEqual(["Beta", "Delta", "Gamma", "Alpha"]);
  });

  it("mergeSourceResults treats unknown trust labels as the lowest tier", () => {
    const catalog = useCatalogStore();
    catalog.mergeSourceResults("registry-a", [
      fixtureEntry({
        id: "weird",
        source: "registry-a",
        display_name: "Weird",
        trust: "experimental"
      }),
      fixtureEntry({
        id: "trusted",
        source: "registry-a",
        display_name: "Trusted",
        trust: "verified"
      })
    ] as never);

    const order = catalog.entries.map((entry) => entry.display_name);
    expect(order).toEqual(["Trusted", "Weird"]);
  });

  it("mergeSourceResults overwrites entries with the same (source, id) key", () => {
    const catalog = useCatalogStore();
    catalog.entries = [
      fixtureEntry({
        id: "filesystem",
        source: "registry-a",
        display_name: "Old",
        trust: "community"
      })
    ] as never;
    catalog.mergeSourceResults("registry-a", [
      fixtureEntry({
        id: "filesystem",
        source: "registry-a",
        display_name: "New",
        trust: "verified"
      })
    ] as never);

    expect(catalog.entries).toHaveLength(1);
    expect(catalog.entries[0].display_name).toBe("New");
    expect(catalog.entries[0].trust).toBe("verified");
  });
});
