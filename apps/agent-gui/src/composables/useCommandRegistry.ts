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
      description: "Switch the active model — pick a profile below",
      context: "session-active",
      insertText: "/model "
    },
    {
      id: "help",
      label: "/help",
      description: "Show available commands and skills",
      context: "always",
      handler: async () => {
        // palette itself serves as the help display
      }
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
