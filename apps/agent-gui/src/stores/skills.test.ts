import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useSkillsStore } from "@/stores/skills";
import type { RemoteSkillSearchResult, SkillSettingsView } from "@/generated/commands";

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
  return {
    id: "review",
    name: "review",
    description: "Review code changes.",
    version: null,
    scope: "user",
    path: "/tmp/review/SKILL.md",
    enabled: true,
    activation_mode: "manual",
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

    await store.setSkillEnabled("review", true);

    expect(store.skillSettings[0].enabled).toBe(false);
    expect(store.error).toContain("state file is read-only");
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

    await store.deleteSkill("review");

    expect(mockedInvoke).toHaveBeenCalledWith("delete_skill_settings", {
      skillId: "review"
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

    await store.deleteSkill("review");

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

  it("installs a remote skill with package source and upserts the result", async () => {
    const installedSkill = createSkillSetting({ id: "review", install_source: "registry" });
    mockedInvoke.mockResolvedValueOnce(installedSkill);

    const store = useSkillsStore();
    const result = await store.installRemoteSkill("@skills/review", "user");

    expect(mockedInvoke).toHaveBeenCalledWith("install_remote_skill", {
      request: {
        package: "@skills/review",
        source: "@skills/review",
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

    const result = await store.updateSkill("review");

    expect(mockedInvoke).toHaveBeenCalledWith("update_skill", { skillId: "review" });
    expect(result).toEqual(updatedSkill);
    expect(store.skillSettings).toEqual([updatedSkill]);
  });

  it("appends a new skill setting after successful update", async () => {
    const existingSkill = createSkillSetting({ id: "planning" });
    const updatedSkill = createSkillSetting({ id: "review", version: "1.1.0" });
    mockedInvoke.mockResolvedValueOnce(updatedSkill);

    const store = useSkillsStore();
    store.skillSettings = [existingSkill];

    const result = await store.updateSkill("review");

    expect(result).toEqual(updatedSkill);
    expect(store.skillSettings).toEqual([existingSkill, updatedSkill]);
  });
});
