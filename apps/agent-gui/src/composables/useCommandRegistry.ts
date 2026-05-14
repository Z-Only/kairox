import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore } from "@/stores/session";
import { useSkillsStore } from "@/stores/skills";

export interface CommandDef {
  id: string;
  label: string;
  description: string;
  /** If set, command executes immediately without inserting text */
  handler?: () => Promise<void>;
  /** If set, text inserted into input when selected (replaces the slash-trigger text) */
  insertText?: string;
  /** Context in which command is available */
  context?: "always" | "session-active";
}

export function useCommandRegistry() {
  const session = useSessionStore();
  const skills = useSkillsStore();

  const builtinCommands: CommandDef[] = [
    {
      id: "clear",
      label: "/clear",
      description: "Clear the current conversation",
      context: "session-active",
      handler: async () => {
        session.resetProjection();
      }
    },
    {
      id: "compact",
      label: "/compact",
      description: "Compact context to save tokens",
      context: "session-active",
      handler: async () => {
        if (session.currentSessionId) {
          await invoke("compact_session");
        }
      }
    },
    {
      id: "model",
      label: "/model",
      description: "Switch the active model",
      context: "session-active",
      insertText: "/model "
    },
    {
      id: "help",
      label: "/help",
      description: "Show available commands",
      insertText: "/help"
    }
  ];

  const filterText = ref("");

  const matchingCommands = computed(() => {
    const q = filterText.value.toLowerCase();
    if (!q) return builtinCommands;

    return builtinCommands.filter(
      (cmd) =>
        cmd.id.toLowerCase().includes(q) ||
        cmd.label.toLowerCase().includes(q) ||
        cmd.description.toLowerCase().includes(q)
    );
  });

  const matchingSkills = computed(() => {
    const q = filterText.value.toLowerCase();
    if (!q) return skills.activeSkills;

    return skills.activeSkills.filter(
      (s) => s.skill_id.toLowerCase().includes(q) || s.name.toLowerCase().includes(q)
    );
  });

  function setFilter(text: string) {
    filterText.value = text;
  }

  function allItems() {
    const items: Array<
      | { kind: "command"; command: CommandDef }
      | { kind: "skill"; skillId: string; displayName: string }
    > = [];

    for (const cmd of matchingCommands.value) {
      if (cmd.context === "session-active" && !session.currentSessionId) continue;
      items.push({ kind: "command", command: cmd });
    }

    for (const skill of matchingSkills.value) {
      items.push({
        kind: "skill",
        skillId: skill.skill_id,
        displayName: skill.name
      });
    }

    return items;
  }

  return {
    filterText,
    matchingCommands,
    matchingSkills,
    setFilter,
    allItems
  };
}
