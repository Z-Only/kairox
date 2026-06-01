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

describe("catalog store — helpers and computeds", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    pushNotificationSpy.mockClear();
  });

  it("reset restores initial state", () => {
    const catalog = useCatalogStore();
    catalog.entries = [fixtureEntry()] as never;
    catalog.installed = [{ server_id: "x" }] as never;
    catalog.installedServerNames = new Set(["x"]);
    catalog.installState = { x: { kind: "installed" } } as never;
    catalog.loading = true;
    catalog.error = "something";
    catalog.tab = "installed";
    catalog.filters = { keyword: "test", category: "ai", trustMin: "verified" };
    catalog.sources = [sampleSource] as never;
    catalog.sourceFailures = { src: "fail" };
    catalog.currentInstallEntryId = "some-entry";

    catalog.reset();

    expect(catalog.entries).toEqual([]);
    expect(catalog.installed).toEqual([]);
    expect(catalog.installedServerNames.size).toBe(0);
    expect(catalog.installState).toEqual({});
    expect(catalog.loading).toBe(false);
    expect(catalog.error).toBeNull();
    expect(catalog.tab).toBe("browse");
    expect(catalog.filters).toEqual({ keyword: "", category: null, trustMin: null });
    expect(catalog.sources).toEqual([]);
    expect(catalog.sourceFailures).toEqual({});
    expect(catalog.currentInstallEntryId).toBeNull();
  });

  it("requestInstallProgress clears stale outcome and sets entryId", () => {
    const catalog = useCatalogStore();
    catalog.installState["entry-1"] = { kind: "installed" } as never;

    catalog.requestInstallProgress("entry-1");

    expect(catalog.installState["entry-1"]).toBeUndefined();
    expect(catalog.currentInstallEntryId).toBe("entry-1");
  });

  it("dismissInstallProgress clears currentInstallEntryId", () => {
    const catalog = useCatalogStore();
    catalog.currentInstallEntryId = "entry-1";

    catalog.dismissInstallProgress();

    expect(catalog.currentInstallEntryId).toBeNull();
  });

  it("hasEntries returns true when entries are populated", () => {
    const catalog = useCatalogStore();
    expect(catalog.hasEntries).toBe(false);
    catalog.entries = [fixtureEntry()] as never;
    expect(catalog.hasEntries).toBe(true);
  });

  it("installedCount returns the length of installed array", () => {
    const catalog = useCatalogStore();
    expect(catalog.installedCount).toBe(0);
    catalog.installed = [{ server_id: "a" }, { server_id: "b" }] as never;
    expect(catalog.installedCount).toBe(2);
  });

  it("allSourceIds includes builtin even if not in sources list", () => {
    const catalog = useCatalogStore();
    catalog.sources = [{ ...sampleSource, id: "custom" }] as never;
    expect(catalog.allSourceIds).toContain("builtin");
    expect(catalog.allSourceIds).toContain("custom");
    expect(catalog.allSourceIds[0]).toBe("builtin");
  });

  it("allSourceIds does not duplicate builtin if already present", () => {
    const catalog = useCatalogStore();
    catalog.sources = [{ ...sampleSource, id: "builtin" }] as never;
    const ids = catalog.allSourceIds;
    expect(ids.filter((id) => id === "builtin")).toHaveLength(1);
  });

  it("isSourceEnabled returns true for builtin when not in sources", () => {
    const catalog = useCatalogStore();
    expect(catalog.isSourceEnabled("builtin")).toBe(true);
  });

  it("isSourceEnabled returns the enabled flag of an existing source", () => {
    const catalog = useCatalogStore();
    catalog.sources = [{ ...sampleSource, id: "custom", enabled: false }] as never;
    expect(catalog.isSourceEnabled("custom")).toBe(false);
  });

  it("isServerInstalled returns true for installed servers", () => {
    const catalog = useCatalogStore();
    catalog.installedServerNames = new Set(["fs", "git"]);
    expect(catalog.isServerInstalled("fs")).toBe(true);
    expect(catalog.isServerInstalled("db")).toBe(false);
  });

  it("visibleEntries only shows entries whose source is enabled", () => {
    const catalog = useCatalogStore();
    catalog.entries = [
      fixtureEntry({ id: "a", source: "builtin" }),
      fixtureEntry({ id: "b", source: "disabled-source" })
    ] as never;
    catalog.sources = [{ ...sampleSource, id: "disabled-source", enabled: false }] as never;

    expect(catalog.visibleEntries.map((e) => e.id)).toEqual(["a"]);
  });

  it("filteredEntries filters by category", () => {
    const catalog = useCatalogStore();
    catalog.entries = [
      fixtureEntry({ id: "a", categories: ["ai"] }),
      fixtureEntry({ id: "b", categories: ["filesystem"] })
    ] as never;
    catalog.filters.category = "ai";
    expect(catalog.filteredEntries.map((e) => e.id)).toEqual(["a"]);
  });
});

describe("catalog store — async action error paths", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
    pushNotificationSpy.mockClear();
  });

  it("fetchCatalog sets error and notifies on rejection", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockRejectedValueOnce(new Error("network error"));
    await catalog.fetchCatalog();
    expect(catalog.error).toContain("network error");
    expect(catalog.loading).toBe(false);
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Failed to load catalog")
    );
  });

  it("fetchInstalled sets error and notifies on rejection", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockRejectedValueOnce(new Error("db offline"));
    await catalog.fetchInstalled();
    expect(catalog.error).toContain("db offline");
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Failed to load installed entries")
    );
  });

  it("checkInstalledStatus silently logs errors without setting store error", async () => {
    const catalog = useCatalogStore();
    const consoleSpy = vi.spyOn(console, "error").mockImplementation(() => {});
    mockedInvoke.mockRejectedValueOnce(new Error("transient"));
    await catalog.checkInstalledStatus();
    expect(catalog.error).toBeNull();
    expect(consoleSpy).toHaveBeenCalled();
    consoleSpy.mockRestore();
  });

  it("checkInstalledStatus populates installedServerNames on success", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockResolvedValueOnce([
      { server_id: "fs", catalog_id: "fs", source: "builtin" }
    ] as never);
    await catalog.checkInstalledStatus();
    expect(catalog.installedServerNames.has("fs")).toBe(true);
    expect(catalog.installed).toHaveLength(1);
  });

  it("getCatalogEntry returns entry on success", async () => {
    const catalog = useCatalogStore();
    const entry = fixtureEntry({ id: "special" });
    mockedInvoke.mockResolvedValueOnce(entry as never);
    const result = await catalog.getCatalogEntry("special", "builtin");
    expect(mockedInvoke).toHaveBeenCalledWith("get_catalog_entry", {
      id: "special",
      source: "builtin"
    });
    expect(result).toEqual(entry);
  });

  it("getCatalogEntry returns null and notifies on failure", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockRejectedValueOnce(new Error("not found"));
    const result = await catalog.getCatalogEntry("missing");
    expect(result).toBeNull();
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Failed to load catalog entry missing")
    );
  });

  it("getCatalogEntry passes null source when not provided", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockResolvedValueOnce(null as never);
    await catalog.getCatalogEntry("x");
    expect(mockedInvoke).toHaveBeenCalledWith("get_catalog_entry", {
      id: "x",
      source: null
    });
  });

  it("installEntry returns null and notifies on failure", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockRejectedValueOnce(new Error("install failed"));
    const result = await catalog.installEntry({
      catalog_id: "broken",
      source: "builtin",
      server_id_override: null,
      env_overrides: {},
      trust_grant: false,
      auto_start: false
    });
    expect(result).toBeNull();
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Failed to install broken")
    );
  });

  it("installEntry does not call fetchInstalled when outcome is not installed", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockResolvedValueOnce({
      kind: "missing_env",
      server_id: null,
      started: false,
      missing_runtimes: [],
      missing_env_keys: ["API_KEY"]
    } as never);
    await catalog.installEntry({
      catalog_id: "partial",
      source: "builtin",
      server_id_override: null,
      env_overrides: {},
      trust_grant: false,
      auto_start: false
    });
    // Only 1 call (install_catalog_entry), not a 2nd for list_installed_entries
    expect(mockedInvoke).toHaveBeenCalledTimes(1);
  });

  it("uninstallEntry notifies on failure", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockRejectedValueOnce(new Error("uninstall err"));
    await catalog.uninstallEntry("broken");
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Failed to uninstall broken")
    );
  });

  it("refreshCatalogSource clears all sourceFailures when source is null", async () => {
    const catalog = useCatalogStore();
    catalog.sourceFailures = { a: "err1", b: "err2" };
    mockedInvoke
      .mockResolvedValueOnce(undefined as never) // refresh_catalog
      .mockResolvedValueOnce([] as never); // list_catalog
    await catalog.refreshCatalogSource(null);
    expect(catalog.sourceFailures).toEqual({});
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "refresh_catalog", { source: null });
  });

  it("refreshCatalogSource notifies on failure", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockRejectedValueOnce(new Error("refresh err"));
    await catalog.refreshCatalogSource("src");
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Failed to refresh catalog")
    );
  });

  it("fetchSources sets error and notifies on failure", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockRejectedValueOnce(new Error("sources err"));
    await catalog.fetchSources();
    expect(catalog.error).toContain("sources err");
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Failed to load catalog sources")
    );
  });

  it("addSource notifies on failure", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockRejectedValueOnce(new Error("add err"));
    await catalog.addSource({
      id: "bad",
      display_name: "Bad",
      kind: "mcp_registry",
      url: "http://bad",
      api_key_env: null,
      priority: 50,
      default_trust: "community",
      enabled: true,
      cache_ttl_seconds: null
    });
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Failed to add source bad")
    );
  });

  it("removeSource notifies on failure", async () => {
    const catalog = useCatalogStore();
    mockedInvoke.mockRejectedValueOnce(new Error("remove err"));
    await catalog.removeSource("bad-id");
    expect(pushNotificationSpy).toHaveBeenCalledWith(
      "error",
      expect.stringContaining("Failed to remove source bad-id")
    );
  });

  it("toggleSource enables a disabled source and refreshes it", async () => {
    const catalog = useCatalogStore();
    catalog.sources = [{ ...sampleSource, id: "custom", enabled: false }] as never;
    mockedInvoke
      .mockResolvedValueOnce(undefined as never) // set_catalog_source_enabled
      .mockResolvedValueOnce([{ ...sampleSource, id: "custom", enabled: true }] as never) // fetchSources
      .mockResolvedValueOnce(undefined as never) // refresh_catalog
      .mockResolvedValueOnce([] as never); // fetchCatalog (from refreshCatalogSource)

    await catalog.toggleSource("custom");

    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "set_catalog_source_enabled", {
      id: "custom",
      enabled: true
    });
    // Should call refresh_catalog for the source since it was re-enabled
    expect(mockedInvoke).toHaveBeenCalledWith("refresh_catalog", { source: "custom" });
  });

  it("toggleSource disables an enabled source and fetches catalog", async () => {
    const catalog = useCatalogStore();
    catalog.sources = [{ ...sampleSource, id: "custom", enabled: true }] as never;
    mockedInvoke
      .mockResolvedValueOnce(undefined as never) // set_catalog_source_enabled
      .mockResolvedValueOnce([{ ...sampleSource, id: "custom", enabled: false }] as never) // fetchSources
      .mockResolvedValueOnce([] as never); // list_catalog (from fetchCatalog)

    await catalog.toggleSource("custom");

    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "set_catalog_source_enabled", {
      id: "custom",
      enabled: false
    });
    // Should call fetchCatalog rather than refreshCatalogSource when disabling
    expect(mockedInvoke).toHaveBeenCalledWith("list_catalog", { query: expect.any(Object) });
  });
});
