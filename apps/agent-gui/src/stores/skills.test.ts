import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { invoke } from "@tauri-apps/api/core";
import { useSkillsStore } from "@/stores/skills";

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
});
