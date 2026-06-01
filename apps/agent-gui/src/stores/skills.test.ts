import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useSkillsStore } from "@/stores/skills";
import type {
  EffectiveSkillView,
  RemoteSkillSearchResult,
  SkillCatalogEntry,
  SkillCatalogQuery,
  SkillSettingsView,
  SkillSourceView
} from "@/generated/commands";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

const mockedInvoke = vi.mocked(invoke);

const discoveredSkill = {
  id: "test-driven-rust",
  name: "test-driven-rust",
  description: "Write Rust changes test-first.",
  version: "1.0.0",
  source: "builtin:/skills/test-driven-rust",
  activation_mode: "manual",
  keywords: ["rust", "tdd"],
  tools: [],
  can_request_tools: [],
  valid: true,
  validation_error: null
};

const activeSkill = {
  skill_id: "test-driven-rust",
  name: "test-driven-rust",
  source: "builtin:/skills/test-driven-rust",
  activation_mode: "manual"
};

function createSkillSetting(overrides: Partial<SkillSettingsView> = {}): SkillSettingsView {
  const id = overrides.id ?? "review";
  const scope = overrides.scope ?? "user";
  return {
    settings_id: `${scope}:${id}`,
    id,
    name: id,
    description: "Review code changes.",
    version: null,
    scope,
    path: `/tmp/${id}/SKILL.md`,
    enabled: true,
    activation_mode: "manual",
    tools: [],
    can_request_tools: [],
    permission_summary: "no tool permissions declared",
    install_source: "local",
    update_state: "unknown",
    effective: true,
    shadowed_by: null,
    valid: true,
    validation_error: null,
    editable: true,
    deletable: true,
    ...overrides
  };
}

function createRemoteSkillResult(
  overrides: Partial<RemoteSkillSearchResult> = {}
): RemoteSkillSearchResult {
  return {
    name: "review",
    description: "Review code changes.",
    repository: "https://github.com/example/review",
    install_count: 42,
    source_url: "https://registry.example/review",
    package: "@skills/review",
    ...overrides
  };
}

function createCatalogEntry(overrides: Partial<SkillCatalogEntry> = {}): SkillCatalogEntry {
  return {
    catalog_id: "skillhub/review",
    name: "Review",
    description: "Review code changes.",
    source: "skillhub",
    source_url: "https://registry.example/review",
    install_count: 42,
    github_stars: 10,
    security_score: 95,
    rating: 4.8,
    package: "skillhub/review",
    package_url: "https://registry.example/download/review",
    ...overrides
  };
}

function createSourceView(overrides: Partial<SkillSourceView> = {}): SkillSourceView {
  return {
    id: "skillhub",
    name: "SkillHub",
    kind: "registry",
    url: "https://skillhub.example",
    enabled: true,
    ...overrides
  };
}

describe("skills store", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("loads discovered and active skills", async () => {
    mockedInvoke.mockResolvedValueOnce([discoveredSkill]).mockResolvedValueOnce([activeSkill]);

    const skills = useSkillsStore();
    await skills.loadSkills();

    expect(mockedInvoke).toHaveBeenCalledWith("list_skills");
    expect(mockedInvoke).toHaveBeenCalledWith("list_active_skills");
    expect(skills.skills[0].name).toBe("test-driven-rust");
    expect(skills.activeSkills[0].skill_id).toBe("test-driven-rust");
    expect(skills.hasSkills).toBe(true);
    expect(skills.loading).toBe(false);
    expect(skills.error).toBeNull();
  });

  it("loads selected skill details", async () => {
    mockedInvoke.mockResolvedValueOnce({
      view: discoveredSkill,
      body_markdown: "# Test Driven Rust"
    });

    const skills = useSkillsStore();
    await skills.loadSkillDetail("test-driven-rust");

    expect(mockedInvoke).toHaveBeenCalledWith("get_skill_detail", {
      skillId: "test-driven-rust"
    });
    expect(skills.selectedSkill?.body_markdown).toContain("Test Driven Rust");
  });

  it("activates a skill and records it as active", async () => {
    mockedInvoke.mockResolvedValueOnce(activeSkill);

    const skills = useSkillsStore();
    await skills.activateSkill("test-driven-rust");

    expect(mockedInvoke).toHaveBeenCalledWith("activate_skill", {
      skillId: "test-driven-rust"
    });
    expect(skills.isSkillActive("test-driven-rust")).toBe(true);
  });

  it("deactivates a skill and removes it from active skills", async () => {
    mockedInvoke.mockResolvedValueOnce(null);

    const skills = useSkillsStore();
    skills.activeSkills = [activeSkill];

    await skills.deactivateSkill("test-driven-rust");

    expect(mockedInvoke).toHaveBeenCalledWith("deactivate_skill", {
      skillId: "test-driven-rust"
    });
    expect(skills.isSkillActive("test-driven-rust")).toBe(false);
  });

  it("stores load errors and clears loading", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("skills unavailable"));

    const skills = useSkillsStore();
    await skills.loadSkills();

    expect(skills.error).toContain("skills unavailable");
    expect(skills.loading).toBe(false);
  });

  it("does not optimistically keep failed skill enablement", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("state file is read-only"));

    const store = useSkillsStore();
    store.skillSettings = [createSkillSetting({ enabled: false })];

    await store.setSkillEnabled("user:review", true);

    expect(store.skillSettings[0].enabled).toBe(false);
    expect(store.error).toContain("state file is read-only");
  });

  it("updates only the matching settings row when duplicate skill ids exist", async () => {
    mockedInvoke.mockResolvedValueOnce(null);

    const store = useSkillsStore();
    store.skillSettings = [
      createSkillSetting({ settings_id: "project:review", scope: "project", enabled: true }),
      createSkillSetting({ settings_id: "user:review", scope: "user", enabled: true })
    ];

    await store.setSkillEnabled("project:review", false);

    expect(mockedInvoke).toHaveBeenCalledWith("set_skill_enabled", {
      skillId: "project:review",
      enabled: false
    });
    expect(store.skillSettings).toEqual([
      createSkillSetting({ settings_id: "project:review", scope: "project", enabled: false }),
      createSkillSetting({ settings_id: "user:review", scope: "user", enabled: true })
    ]);
  });

  it("loads skill settings from generated command envelope", async () => {
    const skillSettings = [createSkillSetting({ id: "review" })];
    mockedInvoke.mockResolvedValueOnce(skillSettings);

    const store = useSkillsStore();
    await store.loadSkillSettings();

    expect(mockedInvoke).toHaveBeenCalledWith("list_skill_settings");
    expect(store.skillSettings).toEqual(skillSettings);
    expect(store.settingsLoading).toBe(false);
    expect(store.error).toBeNull();
  });

  it("deletes skill settings after successful command", async () => {
    mockedInvoke.mockResolvedValueOnce(null);

    const store = useSkillsStore();
    store.skillSettings = [
      createSkillSetting({ id: "review" }),
      createSkillSetting({ id: "planning" })
    ];

    await store.deleteSkill("user:review");

    expect(mockedInvoke).toHaveBeenCalledWith("delete_skill_settings", {
      skillId: "user:review"
    });
    expect(store.skillSettings).toEqual([createSkillSetting({ id: "planning" })]);
  });

  it("keeps skill settings when delete command returns generated error envelope", async () => {
    const existingSkillSettings = [
      createSkillSetting({ id: "review" }),
      createSkillSetting({ id: "planning" })
    ];
    mockedInvoke.mockRejectedValueOnce("delete failed");

    const store = useSkillsStore();
    store.skillSettings = existingSkillSettings;

    await store.deleteSkill("user:review");

    expect(store.error).toContain("delete failed");
    expect(store.skillSettings).toEqual(existingSkillSettings);
  });

  it("updates remote skill results after successful search", async () => {
    const remoteResults = [createRemoteSkillResult({ name: "review" })];
    mockedInvoke.mockResolvedValueOnce(remoteResults);

    const store = useSkillsStore();
    await store.searchRemoteSkills("review");

    expect(mockedInvoke).toHaveBeenCalledWith("search_remote_skills", { query: "review" });
    expect(store.remoteResults).toEqual(remoteResults);
    expect(store.remoteLoading).toBe(false);
  });

  it("preserves remote skill results and clears loading when search fails", async () => {
    const existingResults = [createRemoteSkillResult({ name: "existing" })];
    mockedInvoke.mockRejectedValueOnce(new Error("registry unavailable"));

    const store = useSkillsStore();
    store.remoteResults = existingResults;

    await store.searchRemoteSkills("review");

    expect(store.remoteResults).toEqual(existingResults);
    expect(store.remoteLoading).toBe(false);
    expect(store.error).toContain("registry unavailable");
  });

  it("caches catalog searches and supports forced refresh of the same query", async () => {
    const query: SkillCatalogQuery = { keyword: "review", sources: ["skillhub"], limit: 100 };
    const firstResults = [createCatalogEntry({ name: "Review" })];
    const refreshedResults = [createCatalogEntry({ name: "Review Pro" })];
    mockedInvoke.mockResolvedValueOnce(firstResults).mockResolvedValueOnce(refreshedResults);

    const store = useSkillsStore();
    await store.searchCatalog(query);
    await store.searchCatalog(query);
    await store.searchCatalog(query, { force: true });

    expect(mockedInvoke).toHaveBeenCalledTimes(2);
    expect(mockedInvoke).toHaveBeenNthCalledWith(1, "list_skill_catalog", { query });
    expect(mockedInvoke).toHaveBeenNthCalledWith(2, "list_skill_catalog", { query });
    expect(store.catalogEntries).toEqual(refreshedResults);
  });

  it("installs a remote skill with package source and upserts the result", async () => {
    const installedSkill = createSkillSetting({ id: "review", install_source: "registry" });
    mockedInvoke.mockResolvedValueOnce(installedSkill);

    const store = useSkillsStore();
    const result = await store.installRemoteSkill("@skills/review", "user");

    expect(mockedInvoke).toHaveBeenCalledWith("install_remote_skill", {
      request: {
        package: "@skills/review",
        package_url: null,
        source: "registry",
        target: "user"
      }
    });
    expect(result).toEqual(installedSkill);
    expect(store.skillSettings).toEqual([installedSkill]);
    expect(store.settingsLoading).toBe(false);
  });

  it("returns null and preserves skill settings when remote install fails", async () => {
    const existingSkillSettings = [createSkillSetting({ id: "existing" })];
    mockedInvoke.mockRejectedValueOnce(new Error("install failed"));

    const store = useSkillsStore();
    store.skillSettings = existingSkillSettings;

    const result = await store.installRemoteSkill("@skills/review", "project");

    expect(result).toBeNull();
    expect(store.error).toContain("install failed");
    expect(store.skillSettings).toEqual(existingSkillSettings);
    expect(store.settingsLoading).toBe(false);
  });

  it("installs a GitHub skill and upserts the result", async () => {
    const existingSkill = createSkillSetting({ id: "review", version: "1.0.0" });
    const installedSkill = createSkillSetting({
      id: "review",
      version: "1.1.0",
      install_source: "github"
    });
    mockedInvoke.mockResolvedValueOnce(installedSkill);

    const store = useSkillsStore();
    store.skillSettings = [existingSkill];

    const result = await store.installGithubSkill("https://github.com/example/review", "project");

    expect(mockedInvoke).toHaveBeenCalledWith("install_github_skill", {
      request: {
        source: "https://github.com/example/review",
        target: "project"
      }
    });
    expect(result).toEqual(installedSkill);
    expect(store.skillSettings).toEqual([installedSkill]);
  });

  it("replaces an existing skill setting after successful update", async () => {
    const existingSkill = createSkillSetting({ id: "review", version: "1.0.0" });
    const updatedSkill = createSkillSetting({ id: "review", version: "1.1.0" });
    mockedInvoke.mockResolvedValueOnce(updatedSkill);

    const store = useSkillsStore();
    store.skillSettings = [existingSkill];

    const result = await store.updateSkill("user:review");

    expect(mockedInvoke).toHaveBeenCalledWith("update_skill", { skillId: "user:review" });
    expect(result).toEqual(updatedSkill);
    expect(store.skillSettings).toEqual([updatedSkill]);
  });

  it("appends a new skill setting after successful update", async () => {
    const existingSkill = createSkillSetting({ id: "planning" });
    const updatedSkill = createSkillSetting({ id: "review", version: "1.1.0" });
    mockedInvoke.mockResolvedValueOnce(updatedSkill);

    const store = useSkillsStore();
    store.skillSettings = [existingSkill];

    const result = await store.updateSkill("user:review");

    expect(result).toEqual(updatedSkill);
    expect(store.skillSettings).toEqual([existingSkill, updatedSkill]);
  });

  it("installRemoteSkill passes package_url when provided", async () => {
    const installedSkill = createSkillSetting({ id: "review", install_source: "registry" });
    mockedInvoke.mockResolvedValueOnce(installedSkill);

    const store = useSkillsStore();
    await store.installRemoteSkill(
      "@skills/review",
      "user",
      "https://registry.example/download/review"
    );

    expect(mockedInvoke).toHaveBeenCalledWith("install_remote_skill", {
      request: {
        package: "@skills/review",
        package_url: "https://registry.example/download/review",
        source: "registry",
        target: "user"
      }
    });
  });

  it("installGithubSkill returns null on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("repo not found"));

    const store = useSkillsStore();
    const result = await store.installGithubSkill("https://github.com/bad/repo", "user");

    expect(result).toBeNull();
    expect(store.error).toContain("repo not found");
    expect(store.settingsLoading).toBe(false);
  });

  it("updateSkill returns null and sets error on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("update failed"));

    const store = useSkillsStore();
    store.skillSettings = [createSkillSetting({ id: "review" })];

    const result = await store.updateSkill("user:review");

    expect(result).toBeNull();
    expect(store.error).toContain("update failed");
    expect(store.settingsLoading).toBe(false);
    // Existing settings preserved
    expect(store.skillSettings).toHaveLength(1);
  });

  it("loadSkillSettings sets error and clears loading on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("settings load error"));

    const store = useSkillsStore();
    await store.loadSkillSettings();

    expect(store.error).toContain("settings load error");
    expect(store.settingsLoading).toBe(false);
  });

  it("loadSkillDetail stores error on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("detail failed"));

    const store = useSkillsStore();
    await store.loadSkillDetail("missing-skill");

    expect(store.error).toContain("detail failed");
    expect(store.selectedSkill).toBeNull();
  });

  it("activateSkill stores error and clears activatingSkillId on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("activation failed"));

    const store = useSkillsStore();
    await store.activateSkill("bad-skill");

    expect(store.error).toContain("activation failed");
    expect(store.activatingSkillId).toBeNull();
  });

  it("deactivateSkill stores error and clears activatingSkillId on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("deactivation failed"));

    const store = useSkillsStore();
    store.activeSkills = [activeSkill];

    await store.deactivateSkill("test-driven-rust");

    expect(store.error).toContain("deactivation failed");
    expect(store.activatingSkillId).toBeNull();
    // Active skills not modified on failure
    expect(store.activeSkills).toHaveLength(1);
  });

  it("activateSkill replaces existing active skill entry for same skill_id", async () => {
    const updatedActiveSkill = { ...activeSkill, activation_mode: "auto" };
    mockedInvoke.mockResolvedValueOnce(updatedActiveSkill);

    const store = useSkillsStore();
    store.activeSkills = [activeSkill];

    await store.activateSkill("test-driven-rust");

    expect(store.activeSkills).toHaveLength(1);
    expect(store.activeSkills[0].activation_mode).toBe("auto");
  });
});

function createEffectiveSkill(overrides: Partial<EffectiveSkillView> = {}): EffectiveSkillView {
  return {
    value: createSkillSetting(),
    source: "User",
    overrides: null,
    enabled: true,
    disabledBy: null,
    writable: true,
    deletable: true,
    ...overrides
  };
}

describe("effective skills", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("fetchEffectiveSkills populates effectiveSkills", async () => {
    const effective = createEffectiveSkill();
    mockedInvoke.mockResolvedValueOnce([effective]);

    const store = useSkillsStore();
    await store.fetchEffectiveSkills();

    expect(mockedInvoke).toHaveBeenCalledWith("get_effective_skills");
    expect(store.effectiveSkills).toHaveLength(1);
    expect(store.effectiveSkills[0].source).toBe("User");
    expect(store.effectiveSkills[0].enabled).toBe(true);
    expect(store.effectiveSkills[0].writable).toBe(true);
  });

  it("fetchEffectiveSkills stores error on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("config not available"));

    const store = useSkillsStore();
    await store.fetchEffectiveSkills();

    expect(store.effectiveSkills).toHaveLength(0);
    expect(store.error).toContain("config not available");
  });
});

describe("catalog sources", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    vi.clearAllMocks();
  });

  it("loadCatalogSources populates catalogSources", async () => {
    const source = createSourceView();
    mockedInvoke.mockResolvedValueOnce([source]);

    const store = useSkillsStore();
    await store.loadCatalogSources();

    expect(mockedInvoke).toHaveBeenCalledWith("list_skill_sources");
    expect(store.catalogSources).toEqual([source]);
    expect(store.catalogLoading).toBe(false);
  });

  it("loadCatalogSources stores error on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("sources unavailable"));

    const store = useSkillsStore();
    await store.loadCatalogSources();

    expect(store.error).toContain("sources unavailable");
    expect(store.catalogLoading).toBe(false);
  });

  it("addCatalogSource invokes command and reloads sources", async () => {
    const source = createSourceView({ id: "new-source" });
    mockedInvoke
      .mockResolvedValueOnce(null) // addSkillSource
      .mockResolvedValueOnce([source]); // listSkillSources

    const store = useSkillsStore();
    await store.addCatalogSource(source);

    expect(mockedInvoke).toHaveBeenCalledWith("add_skill_source", { config: source });
    expect(store.catalogSources).toEqual([source]);
    expect(store.catalogLoading).toBe(false);
  });

  it("addCatalogSource re-throws error on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("bad source"));

    const store = useSkillsStore();
    await expect(store.addCatalogSource(createSourceView())).rejects.toThrow("bad source");

    expect(store.error).toContain("bad source");
    expect(store.catalogLoading).toBe(false);
  });

  it("removeCatalogSource removes the source from local state", async () => {
    mockedInvoke.mockResolvedValueOnce(null);

    const store = useSkillsStore();
    store.catalogSources = [createSourceView({ id: "keep" }), createSourceView({ id: "remove" })];

    await store.removeCatalogSource("remove");

    expect(mockedInvoke).toHaveBeenCalledWith("remove_skill_source", { id: "remove" });
    expect(store.catalogSources).toEqual([createSourceView({ id: "keep" })]);
    expect(store.catalogLoading).toBe(false);
  });

  it("removeCatalogSource re-throws error on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("cannot remove"));

    const store = useSkillsStore();
    store.catalogSources = [createSourceView({ id: "src" })];

    await expect(store.removeCatalogSource("src")).rejects.toThrow("cannot remove");
    expect(store.error).toContain("cannot remove");
    expect(store.catalogLoading).toBe(false);
  });

  it("isCatalogSourceEnabled returns true when source is enabled", () => {
    const store = useSkillsStore();
    store.catalogSources = [
      createSourceView({ id: "enabled-src", enabled: true }),
      createSourceView({ id: "disabled-src", enabled: false })
    ];

    expect(store.isCatalogSourceEnabled("enabled-src")).toBe(true);
    expect(store.isCatalogSourceEnabled("disabled-src")).toBe(false);
    expect(store.isCatalogSourceEnabled("nonexistent")).toBe(false);
  });

  it("toggleCatalogSource toggles from enabled to disabled", async () => {
    mockedInvoke.mockResolvedValueOnce(null); // setSkillSourceEnabled

    const store = useSkillsStore();
    store.catalogSources = [createSourceView({ id: "src", enabled: true })];

    await store.toggleCatalogSource("src");

    expect(mockedInvoke).toHaveBeenCalledWith("set_skill_source_enabled", {
      id: "src",
      enabled: false
    });
    expect(store.catalogSources[0].enabled).toBe(false);
  });

  it("toggleCatalogSource toggles from disabled to enabled", async () => {
    mockedInvoke.mockResolvedValueOnce(null); // setSkillSourceEnabled

    const store = useSkillsStore();
    store.catalogSources = [createSourceView({ id: "src", enabled: false })];

    await store.toggleCatalogSource("src");

    expect(mockedInvoke).toHaveBeenCalledWith("set_skill_source_enabled", {
      id: "src",
      enabled: true
    });
    expect(store.catalogSources[0].enabled).toBe(true);
  });

  it("toggleCatalogSource is a no-op for unknown source", async () => {
    const store = useSkillsStore();
    store.catalogSources = [];

    await store.toggleCatalogSource("unknown");

    expect(mockedInvoke).not.toHaveBeenCalled();
  });

  it("setCatalogSourceEnabled updates local state on success", async () => {
    mockedInvoke.mockResolvedValueOnce(null);

    const store = useSkillsStore();
    store.catalogSources = [createSourceView({ id: "src", enabled: true })];

    await store.setCatalogSourceEnabled("src", false);

    expect(mockedInvoke).toHaveBeenCalledWith("set_skill_source_enabled", {
      id: "src",
      enabled: false
    });
    expect(store.catalogSources[0].enabled).toBe(false);
  });

  it("setCatalogSourceEnabled stores error on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("toggle err"));

    const store = useSkillsStore();
    store.catalogSources = [createSourceView({ id: "src", enabled: true })];

    await store.setCatalogSourceEnabled("src", false);

    expect(store.error).toContain("toggle err");
    // State not updated on failure
    expect(store.catalogSources[0].enabled).toBe(true);
  });

  it("refreshCatalog calls command and clears search cache", async () => {
    mockedInvoke
      .mockResolvedValueOnce([createCatalogEntry()]) // searchCatalog initial
      .mockResolvedValueOnce(null); // refreshSkillCatalog

    const store = useSkillsStore();
    // Populate cache
    await store.searchCatalog({ keyword: "test" });
    expect(store.catalogEntries).toHaveLength(1);

    await store.refreshCatalog();

    expect(mockedInvoke).toHaveBeenCalledWith("refresh_skill_catalog");
    expect(store.catalogLoading).toBe(false);
  });

  it("refreshCatalog stores error on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("refresh err"));

    const store = useSkillsStore();
    await store.refreshCatalog();

    expect(store.error).toContain("refresh err");
    expect(store.catalogLoading).toBe(false);
  });

  it("searchCatalog stores error on failure", async () => {
    mockedInvoke.mockRejectedValueOnce(new Error("search failed"));

    const store = useSkillsStore();
    await store.searchCatalog({ keyword: "test" });

    expect(store.error).toContain("search failed");
    expect(store.catalogLoading).toBe(false);
  });
});
