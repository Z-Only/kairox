import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import commandRegistrySource from "./useCommandRegistry.ts?raw";

// Plain mutable object so individual tests can toggle session presence.
// (The mock returns a plain POJO, so Vue ref auto-unwrap does NOT apply;
// a Ref object would always be truthy, breaking the null check in allItems().)
const sessionStore = {
  currentSessionId: "ses_1" as string | null,
  resetProjection: vi.fn(),
  profileInfos: [] as { alias: string; provider: string; model_id: string }[]
};

// Mock Tauri invoke (compact command handler uses it)
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn()
}));

vi.mock("@/stores/session", () => ({
  useSessionStore: () => sessionStore,
  formatProfileDisplay: (p: { alias: string }) => p.alias
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
    it("localizes builtin command descriptions through the provided translator", () => {
      const t = (key: string) =>
        ({
          "chat.commands.clear.description": "清空当前对话",
          "chat.commands.compact.description": "压缩上下文以节省 token",
          "chat.commands.model.description": "切换当前模型",
          "chat.commands.help.description": "显示可用命令和技能"
        })[key] ?? key;
      const registry = useCommandRegistry(t);
      registry.setFilter("");

      const descriptions = registry
        .allItems()
        .filter((item) => item.kind === "command")
        .map((item) => (item.kind === "command" ? item.command.description : ""));

      expect(descriptions).toEqual([
        "清空当前对话",
        "压缩上下文以节省 token",
        "切换当前模型",
        "显示可用命令和技能"
      ]);
    });

    it("does not keep builtin command descriptions inline in the registry source", () => {
      expect(commandRegistrySource).not.toContain("Clear the current conversation");
      expect(commandRegistrySource).not.toContain("Compact context to save tokens");
      expect(commandRegistrySource).not.toContain("Switch the active model");
      expect(commandRegistrySource).not.toContain("Show available commands and skills");
    });

    it("help command has handler and always context", () => {
      const registry = useCommandRegistry();
      registry.setFilter("help");
      const items = registry.allItems();
      expect(items.length).toBe(1);
      expect(items[0].kind).toBe("command");
      if (items[0].kind === "command") {
        expect(items[0].command.handler).toBeDefined();
        expect(items[0].command.context).toBe("always");
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

  describe("model profiles", () => {
    it("includes model-profile items in allItems when profiles are available", () => {
      sessionStore.profileInfos = [
        { alias: "fast", provider: "anthropic", model_id: "claude-3.5-sonnet" },
        { alias: "smart", provider: "openai", model_id: "gpt-4o" }
      ];
      const registry = useCommandRegistry();
      registry.setFilter("");
      const items = registry.allItems();
      const profileItems = items.filter((i) => i.kind === "model-profile");
      expect(profileItems.length).toBe(2);
      expect(profileItems[0]).toEqual({
        kind: "model-profile",
        alias: "fast",
        displayName: expect.any(String)
      });
    });

    it("filters model profiles by alias", () => {
      sessionStore.profileInfos = [
        { alias: "fast", provider: "anthropic", model_id: "claude-3.5-sonnet" },
        { alias: "smart", provider: "openai", model_id: "gpt-4o" }
      ];
      const registry = useCommandRegistry();
      registry.setFilter("fast");
      const items = registry.allItems();
      const profileItems = items.filter((i) => i.kind === "model-profile");
      expect(profileItems.length).toBe(1);
      expect(profileItems[0].alias).toBe("fast");
    });
  });
});
