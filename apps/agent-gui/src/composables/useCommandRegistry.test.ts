import { describe, it, expect, vi, beforeEach } from "vitest";
import { setActivePinia, createPinia } from "pinia";
import commandRegistrySource from "./useCommandRegistry.ts?raw";
import { expectSourceMigration } from "@/test-utils/sourceGuards";

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
    skills: [
      {
        id: "code-review",
        name: "Code Review",
        description: "Review code changes",
        source: "project",
        activation_mode: "manual",
        keywords: [],
        tools: [],
        can_request_tools: [],
        valid: true,
        validation_error: null
      },
      {
        id: "test-gen",
        name: "Test Generator",
        description: "Generate tests",
        source: "project",
        activation_mode: "manual",
        keywords: [],
        tools: [],
        can_request_tools: [],
        valid: true,
        validation_error: null
      }
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
      const commandItems = items.filter((i) => i.kind === "command");
      expect(commandItems.map((item) => (item.kind === "command" ? item.command.id : ""))).toEqual([
        "clear",
        "compact",
        "model",
        "help",
        "instructions",
        "hooks",
        "skills",
        "agents",
        "plugins",
        "mcp",
        "models"
      ]);
    });

    it("returns discovered skills in allItems before they are active", () => {
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
      expect(commandItems.map((item) => (item.kind === "command" ? item.command.id : ""))).toEqual([
        "help",
        "instructions",
        "hooks",
        "skills",
        "agents",
        "plugins",
        "mcp",
        "models"
      ]);
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
          "chat.commands.help.description": "显示可用命令和技能",
          "chat.commands.instructions.description": "打开指令设置",
          "chat.commands.hooks.description": "打开钩子设置",
          "chat.commands.skills.description": "打开技能设置",
          "chat.commands.agents.description": "打开代理设置",
          "chat.commands.plugins.description": "打开插件设置",
          "chat.commands.mcp.description": "打开 MCP 设置",
          "chat.commands.models.description": "打开模型设置"
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
        "显示可用命令和技能",
        "打开指令设置",
        "打开钩子设置",
        "打开技能设置",
        "打开代理设置",
        "打开插件设置",
        "打开 MCP 设置",
        "打开模型设置"
      ]);
    });

    it("navigates settings slash commands through the injected route handler", async () => {
      const navigateToRoute = vi.fn();
      const registry = useCommandRegistry((key) => key, { navigateToRoute });
      registry.setFilter("hooks");

      const [item] = registry.allItems();
      expect(item.kind).toBe("command");
      if (item.kind === "command") {
        await item.command.handler?.();
      }

      expect(navigateToRoute).toHaveBeenCalledWith("settings-hooks");
    });

    it("does not keep builtin command descriptions inline in the registry source", () => {
      expectSourceMigration(commandRegistrySource, {
        forbidden: [
          "Clear the current conversation",
          "Compact context to save tokens",
          "Switch the active model",
          "Show available commands and skills"
        ]
      });
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

  describe("command handler execution", () => {
    it("clear command calls resetProjection on session store", async () => {
      const registry = useCommandRegistry();
      registry.setFilter("clear");
      const items = registry.allItems();
      const clearItem = items.find((i) => i.kind === "command" && i.command.id === "clear");
      expect(clearItem?.kind).toBe("command");
      if (clearItem?.kind === "command") {
        await clearItem.command.handler?.();
      }
      expect(sessionStore.resetProjection).toHaveBeenCalled();
    });

    it("compact command invokes compact_session when session is active", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      sessionStore.currentSessionId = "ses_1";
      const registry = useCommandRegistry();
      registry.setFilter("compact");
      const items = registry.allItems();
      const compactItem = items.find((i) => i.kind === "command" && i.command.id === "compact");
      if (compactItem?.kind === "command") {
        await compactItem.command.handler?.();
      }
      expect(invoke).toHaveBeenCalledWith("compact_session");
    });

    it("compact command does not invoke when session is null", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockClear();
      sessionStore.currentSessionId = null;
      const registry = useCommandRegistry();
      // Manually get all commands without session filter (use matchingCommands directly)
      registry.setFilter("compact");
      const commands = registry.matchingCommands.value;
      const compactCmd = commands.find((c) => c.id === "compact");
      if (compactCmd) {
        await compactCmd.handler?.();
      }
      expect(invoke).not.toHaveBeenCalled();
    });

    it("help command handler executes without error", async () => {
      const registry = useCommandRegistry();
      registry.setFilter("help");
      const items = registry.allItems();
      const helpItem = items.find((i) => i.kind === "command" && i.command.id === "help");
      if (helpItem?.kind === "command") {
        // help handler is a no-op palette display; should not throw
        await expect(helpItem.command.handler?.()).resolves.toBeUndefined();
      }
    });

    it("model command has insertText instead of handler", () => {
      const registry = useCommandRegistry();
      registry.setFilter("model");
      const items = registry.allItems();
      const modelItem = items.find((i) => i.kind === "command" && i.command.id === "model");
      expect(modelItem?.kind).toBe("command");
      if (modelItem?.kind === "command") {
        expect(modelItem.command.handler).toBeUndefined();
        expect(modelItem.command.insertText).toBe("/model ");
      }
    });

    it("navigates to each settings route through handler", async () => {
      const navigateToRoute = vi.fn();
      const registry = useCommandRegistry((key) => key, { navigateToRoute });

      const settingsCommands = [
        { id: "instructions", route: "settings-instructions" },
        { id: "skills", route: "settings-skills" },
        { id: "agents", route: "settings-agents" },
        { id: "plugins", route: "settings-plugins" },
        { id: "mcp", route: "settings-mcp" },
        { id: "models", route: "settings-models" }
      ];

      for (const { id, route } of settingsCommands) {
        navigateToRoute.mockClear();
        registry.setFilter(id);
        const items = registry.allItems();
        const item = items.find((i) => i.kind === "command" && i.command.id === id);
        if (item?.kind === "command") {
          await item.command.handler?.();
        }
        expect(navigateToRoute).toHaveBeenCalledWith(route);
      }
    });
  });

  describe("filtering edge cases", () => {
    it("filters commands by description text", () => {
      const t = (key: string) =>
        key === "chat.commands.clear.description" ? "Clear conversation history" : key;
      const registry = useCommandRegistry(t);
      registry.setFilter("history");
      const items = registry.allItems();
      const commandItems = items.filter((i) => i.kind === "command");
      expect(commandItems.length).toBe(1);
      if (commandItems[0].kind === "command") {
        expect(commandItems[0].command.id).toBe("clear");
      }
    });

    it("filters model profiles by provider", () => {
      sessionStore.profileInfos = [
        { alias: "fast", provider: "anthropic", model_id: "claude-3.5-sonnet" },
        { alias: "smart", provider: "openai", model_id: "gpt-4o" }
      ];
      const registry = useCommandRegistry();
      registry.setFilter("anthropic");
      const items = registry.allItems();
      const profileItems = items.filter((i) => i.kind === "model-profile");
      expect(profileItems.length).toBe(1);
    });

    it("filters model profiles by model_id", () => {
      sessionStore.profileInfos = [
        { alias: "fast", provider: "anthropic", model_id: "claude-3.5-sonnet" },
        { alias: "smart", provider: "openai", model_id: "gpt-4o" }
      ];
      const registry = useCommandRegistry();
      registry.setFilter("gpt-4o");
      const items = registry.allItems();
      const profileItems = items.filter((i) => i.kind === "model-profile");
      expect(profileItems.length).toBe(1);
      expect(profileItems[0].alias).toBe("smart");
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
