import { ref, computed } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useSessionStore, formatProfileDisplay } from "@/stores/session";
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

export type CommandTranslator = (key: string) => string;

interface BuiltinCommandDef extends Omit<CommandDef, "description"> {
  descriptionKey: string;
}

export function useCommandRegistry(t: CommandTranslator = (key) => key) {
  const session = useSessionStore();
  const skills = useSkillsStore();

  const builtinCommandDefs: BuiltinCommandDef[] = [
    {
      id: "clear",
      label: "/clear",
      descriptionKey: "chat.commands.clear.description",
      context: "session-active",
      handler: async () => {
        session.resetProjection();
      }
    },
    {
      id: "compact",
      label: "/compact",
      descriptionKey: "chat.commands.compact.description",
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
      descriptionKey: "chat.commands.model.description",
      context: "session-active",
      insertText: "/model "
    },
    {
      id: "help",
      label: "/help",
      descriptionKey: "chat.commands.help.description",
      context: "always",
      handler: async () => {
        // palette itself serves as the help display
      }
    }
  ];

  const filterText = ref("");

  const builtinCommands = computed<CommandDef[]>(() =>
    builtinCommandDefs.map(({ descriptionKey, ...command }) => ({
      ...command,
      description: t(descriptionKey)
    }))
  );

  const matchingCommands = computed(() => {
    const q = filterText.value.toLowerCase();
    if (!q) return builtinCommands.value;

    return builtinCommands.value.filter(
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

  const matchingProfiles = computed(() => {
    const q = filterText.value.toLowerCase();
    const profiles = session.profileInfos;
    if (!q) return profiles;

    return profiles.filter(
      (p) =>
        p.alias.toLowerCase().includes(q) ||
        (p.provider_display ?? p.provider).toLowerCase().includes(q) ||
        (p.model_display ?? p.model_id).toLowerCase().includes(q)
    );
  });

  function setFilter(text: string) {
    filterText.value = text;
  }

  function allItems() {
    const items: Array<
      | { kind: "command"; command: CommandDef }
      | { kind: "skill"; skillId: string; displayName: string }
      | { kind: "model-profile"; alias: string; displayName: string }
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

    for (const profile of matchingProfiles.value) {
      items.push({
        kind: "model-profile",
        alias: profile.alias,
        displayName: formatProfileDisplay(profile)
      });
    }

    return items;
  }

  return {
    filterText,
    matchingCommands,
    matchingSkills,
    matchingProfiles,
    setFilter,
    allItems
  };
}
