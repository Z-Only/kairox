import { describe, it, expect, beforeEach, vi } from "vitest";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

vi.mock("../composables/useNotifications", () => ({
  addNotification: vi.fn(),
  dismissNotification: vi.fn(),
  notifications: []
}));

import { invoke } from "@tauri-apps/api/core";
const mockedInvoke = vi.mocked(invoke);

import {
  catalogState,
  filteredEntries,
  fetchCatalog,
  fetchInstalled,
  installEntry,
  uninstallEntry,
  refreshCatalogSource,
  resetCatalogState,
  fetchSources,
  addSource,
  removeSource,
  setSourceEnabled,
  handleSourceFailed
} from "./catalog";

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
    resetCatalogState();
    vi.clearAllMocks();
  });

  it("loads entries via list_catalog", async () => {
    mockedInvoke.mockResolvedValueOnce([fixtureEntry()] as never);
    await fetchCatalog();
    expect(mockedInvoke).toHaveBeenCalledWith("list_catalog", {
      query: expect.any(Object)
    });
    expect(catalogState.entries.length).toBe(1);
    expect(catalogState.entries[0].id).toBe("filesystem");
  });

  it("install dispatches install_catalog_entry and stores outcome", async () => {
    mockedInvoke
      .mockResolvedValueOnce({
        kind: "installed",
        server_id: "filesystem",
        started: true,
        missing_runtimes: [],
        missing_env_keys: []
      } as never)
      .mockResolvedValueOnce([] as never); // refreshInstalled

    const outcome = await installEntry({
      catalog_id: "filesystem",
      source: "builtin",
      server_id_override: null,
      env_overrides: { WORKSPACE_PATH: "/tmp" },
      trust_grant: true,
      auto_start: true
    });

    expect(outcome.kind).toBe("installed");
    expect(catalogState.installState["filesystem"]).toEqual({
      kind: "installed",
      server_id: "filesystem",
      started: true,
      missing_runtimes: [],
      missing_env_keys: []
    });
  });

  it("filters by keyword + trust client-side", () => {
    catalogState.entries = [
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
    catalogState.filters.keyword = "alpha";
    catalogState.filters.trustMin = "verified";
    expect(filteredEntries.value.map((e) => e.id)).toEqual(["a"]);
  });

  it("uninstall removes from installState and refreshes installed", async () => {
    catalogState.installState["filesystem"] = {
      kind: "installed",
      server_id: "filesystem",
      started: true,
      missing_runtimes: [],
      missing_env_keys: []
    };
    mockedInvoke
      .mockResolvedValueOnce(undefined as never) // uninstall_catalog_entry
      .mockResolvedValueOnce([] as never); // list_installed_entries

    await uninstallEntry("filesystem");

    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "uninstall_catalog_entry", {
      serverId: "filesystem"
    });
    expect(catalogState.installState["filesystem"]).toBeUndefined();
  });

  it("refreshCatalogSource calls refresh_catalog then re-fetches", async () => {
    mockedInvoke
      .mockResolvedValueOnce(undefined as never) // refresh_catalog
      .mockResolvedValueOnce([] as never); // list_catalog

    await refreshCatalogSource("builtin");

    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "refresh_catalog", {
      source: "builtin"
    });
    expect(mockedInvoke).toHaveBeenNthCalledWith(2, "list_catalog", {
      query: expect.any(Object)
    });
  });

  it("fetchInstalled populates installed list", async () => {
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
    await fetchInstalled();
    expect(catalogState.installed.length).toBe(1);
    expect(catalogState.installed[0].server_id).toBe("filesystem");
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
    resetCatalogState();
    vi.clearAllMocks();
  });

  it("fetchSources loads sources via list_catalog_sources", async () => {
    mockedInvoke.mockResolvedValueOnce([sampleSource] as never);
    await fetchSources();
    expect(mockedInvoke).toHaveBeenCalledWith("list_catalog_sources");
    expect(catalogState.sources).toHaveLength(1);
    expect(catalogState.sources[0].id).toBe("smithery");
  });

  it("addSource calls add_catalog_source then re-fetches", async () => {
    mockedInvoke
      .mockResolvedValueOnce(undefined as never)
      .mockResolvedValueOnce([sampleSource] as never);
    await addSource({
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
    expect(catalogState.sources).toHaveLength(1);
  });

  it("removeSource calls remove_catalog_source then re-fetches", async () => {
    catalogState.sources = [sampleSource];
    mockedInvoke.mockResolvedValueOnce(undefined as never).mockResolvedValueOnce([] as never);
    await removeSource("smithery");
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "remove_catalog_source", {
      id: "smithery"
    });
    expect(catalogState.sources).toHaveLength(0);
  });

  it("setSourceEnabled toggles a source and re-fetches", async () => {
    catalogState.sources = [sampleSource];
    mockedInvoke
      .mockResolvedValueOnce(undefined as never)
      .mockResolvedValueOnce([{ ...sampleSource, enabled: false }] as never);
    await setSourceEnabled("smithery", false);
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "set_catalog_source_enabled", {
      id: "smithery",
      enabled: false
    });
    expect(catalogState.sources[0].enabled).toBe(false);
  });

  it("handleSourceFailed records sourceFailures keyed by source id", () => {
    handleSourceFailed("smithery", "timeout");
    expect(catalogState.sourceFailures.smithery).toBe("timeout");
  });
});
