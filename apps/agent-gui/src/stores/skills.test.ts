import { describe, it, expect, beforeEach, vi } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import { commands } from "@/generated/commands";
import { useSkillsStore } from "@/stores/skills";

vi.mock("@/generated/commands", () => ({
  commands: {
    listSkills: vi.fn(),
    listActiveSkills: vi.fn(),
    getSkillDetail: vi.fn(),
    activateSkill: vi.fn(),
    deactivateSkill: vi.fn()
  }
}));

const mockedCommands = vi.mocked(commands);

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
    mockedCommands.listSkills.mockResolvedValueOnce([discoveredSkill]);
    mockedCommands.listActiveSkills.mockResolvedValueOnce([activeSkill]);

    const skills = useSkillsStore();
    await skills.loadSkills();

    expect(mockedCommands.listSkills).toHaveBeenCalledTimes(1);
    expect(mockedCommands.listActiveSkills).toHaveBeenCalledTimes(1);
    expect(skills.skills[0].name).toBe("test-driven-rust");
    expect(skills.activeSkills[0].skill_id).toBe("test-driven-rust");
    expect(skills.hasSkills).toBe(true);
    expect(skills.loading).toBe(false);
    expect(skills.error).toBeNull();
  });

  it("loads selected skill details", async () => {
    mockedCommands.getSkillDetail.mockResolvedValueOnce({
      view: discoveredSkill,
      body_markdown: "# Test Driven Rust"
    });

    const skills = useSkillsStore();
    await skills.loadSkillDetail("test-driven-rust");

    expect(mockedCommands.getSkillDetail).toHaveBeenCalledWith("test-driven-rust");
    expect(skills.selectedSkill?.body_markdown).toContain("Test Driven Rust");
  });

  it("activates a skill and records it as active", async () => {
    mockedCommands.activateSkill.mockResolvedValueOnce(activeSkill);

    const skills = useSkillsStore();
    await skills.activateSkill("test-driven-rust");

    expect(mockedCommands.activateSkill).toHaveBeenCalledWith("test-driven-rust");
    expect(skills.isSkillActive("test-driven-rust")).toBe(true);
  });

  it("deactivates a skill and removes it from active skills", async () => {
    mockedCommands.deactivateSkill.mockResolvedValueOnce(null);

    const skills = useSkillsStore();
    skills.activeSkills = [activeSkill];

    await skills.deactivateSkill("test-driven-rust");

    expect(mockedCommands.deactivateSkill).toHaveBeenCalledWith("test-driven-rust");
    expect(skills.isSkillActive("test-driven-rust")).toBe(false);
  });

  it("stores load errors and clears loading", async () => {
    mockedCommands.listSkills.mockRejectedValueOnce(new Error("skills unavailable"));

    const skills = useSkillsStore();
    await skills.loadSkills();

    expect(skills.error).toContain("skills unavailable");
    expect(skills.loading).toBe(false);
  });
});
