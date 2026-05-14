import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";

// Plain mutable object so individual tests can toggle session presence.
// (The mock returns a plain POJO, so Vue ref auto-unwrap does NOT apply;
// a Ref object would always be truthy, breaking the null check in allItems().)
const sessionStore = {
  currentSessionId: "ses_1" as string | null,
  resetProjection: vi.fn()
};

// Mock Tauri invoke (compact command handler uses it)
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

vi.mock("@/stores/session", () => ({
  useSessionStore: () => sessionStore
}));

vi.mock("@/stores/skills", () => ({
  useSkillsStore: () => ({
    activeSkills: [
      {
        skill_id: "code-review",
        name: "Code Review",
        source: "project",
        activation_mode: "manual"
      },
      { skill_id: "test-gen", name: "Test Generator", source: "project", activation_mode: "manual" }
    ]
  })
}));

// Dynamic import so mocks are applied before the module-under-test is loaded
const { useCommandRegistry } = await import("./useCommandRegistry");

describe("useCommandRegistry", () => {
  beforeEach(() => {
    setActivePinia(createPinia());
    sessionStore.currentSessionId = "ses_1";
    sessionStore.resetProjection = vi.fn();
  });

  describe("allItems", () => {
    it("returns all builtin commands when no filter and session active", () => {
      const registry = useCommandRegistry();
      registry.setFilter("");
      const items = registry.allItems();
      // 4 builtins: clear, compact, model, help
      const commandItems = items.filter((i) => i.kind === "command");
      expect(commandItems.length).toBe(4);
    });

    it("returns skills in allItems", () => {
      const registry = useCommandRegistry();
      registry.setFilter("");
      const items = registry.allItems();
      const skillItems = items.filter((i) => i.kind === "skill");
      expect(skillItems.length).toBe(2);
    });

    it("returns skill items with correct shape", () => {
      const registry = useCommandRegistry();
      registry.setFilter("");
      const items = registry.allItems();
      const skillItems = items.filter((i) => i.kind === "skill");
      expect(skillItems[0]).toEqual({
        kind: "skill",
        skillId: "code-review",
        displayName: "Code Review"
      });
    });

    it("filters commands by id", () => {
      const registry = useCommandRegistry();
      registry.setFilter("clear");
      const items = registry.allItems();
      expect(items.length).toBe(1);
      expect(items[0].kind).toBe("command");
    });

    it("filters skills by name", () => {
      const registry = useCommandRegistry();
      registry.setFilter("review");
      const items = registry.allItems();
      const skillItems = items.filter((i) => i.kind === "skill");
      expect(skillItems.length).toBe(1);
      expect(skillItems[0].skillId).toBe("code-review");
    });

    it("filters skills by skill_id", () => {
      const registry = useCommandRegistry();
      registry.setFilter("test-gen");
      const items = registry.allItems();
      const skillItems = items.filter((i) => i.kind === "skill");
      expect(skillItems.length).toBe(1);
      expect(skillItems[0].skillId).toBe("test-gen");
    });

    it("excludes session-only commands when no session", () => {
      sessionStore.currentSessionId = null;
      const registry = useCommandRegistry();
      registry.setFilter("");
      const items = registry.allItems();
      const commandItems = items.filter((i) => i.kind === "command");
      // Only "help" has context: "always"
      expect(commandItems.length).toBe(1);
      expect(commandItems[0].kind).toBe("command");
      if (commandItems[0].kind === "command") {
        expect(commandItems[0].command.id).toBe("help");
      }
    });
  });

  describe("setFilter", () => {
    it("updates matchingCommands reactively", () => {
      const registry = useCommandRegistry();
      registry.setFilter("clear");
      // clear command should match by id
      const items = registry.allItems();
      expect(items.length).toBe(1);
    });
  });

  describe("command shape", () => {
    it("help command has insertText but no handler", () => {
      const registry = useCommandRegistry();
      registry.setFilter("help");
      const items = registry.allItems();
      expect(items.length).toBe(1);
      expect(items[0].kind).toBe("command");
      if (items[0].kind === "command") {
        expect(items[0].command.handler).toBeUndefined();
        expect(items[0].command.insertText).toBe("/help");
      }
    });

    it("clear command has handler and session-active context", () => {
      const registry = useCommandRegistry();
      registry.setFilter("clear");
      const items = registry.allItems();
      expect(items.length).toBe(1);
      expect(items[0].kind).toBe("command");
      if (items[0].kind === "command") {
        expect(items[0].command.handler).toBeDefined();
        expect(items[0].command.context).toBe("session-active");
      }
    });
  });
});
