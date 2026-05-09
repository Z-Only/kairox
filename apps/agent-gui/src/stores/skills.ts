// `unplugin-auto-import` only injects globals into `.vue` SFCs. Pinia stores
// are plain `.ts` modules and must import Vue and Pinia APIs explicitly.
import { computed, ref } from "vue";
import { defineStore } from "pinia";
import { invoke } from "@tauri-apps/api/core";

export interface SkillView {
  id: string;
  name: string;
  description: string;
  version: string | null;
  source: string;
  activation_mode: string;
  keywords: string[];
  tools: string[];
  can_request_tools: string[];
  valid: boolean;
  validation_error: string | null;
}

export interface SkillDetail {
  view: SkillView;
  body_markdown: string;
}

export interface ActiveSkillView {
  skill_id: string;
  name: string;
  source: string;
  activation_mode: string;
}

function formatError(caughtError: unknown): string {
  return caughtError instanceof Error ? caughtError.message : String(caughtError);
}

export const useSkillsStore = defineStore("skills", () => {
  const skills = ref<SkillView[]>([]);
  const activeSkills = ref<ActiveSkillView[]>([]);
  const selectedSkill = ref<SkillDetail | null>(null);
  const loading = ref(false);
  const activatingSkillId = ref<string | null>(null);
  const error = ref<string | null>(null);

  const hasSkills = computed(() => skills.value.length > 0);
  const activeSkillIds = computed(() =>
    activeSkills.value.map((activeSkill) => activeSkill.skill_id)
  );

  function isSkillActive(skillId: string): boolean {
    return activeSkillIds.value.includes(skillId);
  }

  async function loadSkills(): Promise<void> {
    loading.value = true;
    error.value = null;
    try {
      const [discoveredSkills, activeSkillViews] = await Promise.all([
        invoke<SkillView[]>("list_skills"),
        invoke<ActiveSkillView[]>("list_active_skills")
      ]);
      skills.value = discoveredSkills;
      activeSkills.value = activeSkillViews;
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      loading.value = false;
    }
  }

  async function loadSkillDetail(skillId: string): Promise<void> {
    error.value = null;
    try {
      selectedSkill.value = await invoke<SkillDetail>("get_skill_detail", { skillId });
    } catch (caughtError) {
      error.value = formatError(caughtError);
    }
  }

  async function activateSkill(skillId: string): Promise<void> {
    activatingSkillId.value = skillId;
    error.value = null;
    try {
      const activeSkill = await invoke<ActiveSkillView>("activate_skill", { skillId });
      activeSkills.value = [
        ...activeSkills.value.filter((existingSkill) => existingSkill.skill_id !== skillId),
        activeSkill
      ];
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      activatingSkillId.value = null;
    }
  }

  async function deactivateSkill(skillId: string): Promise<void> {
    activatingSkillId.value = skillId;
    error.value = null;
    try {
      await invoke("deactivate_skill", { skillId });
      activeSkills.value = activeSkills.value.filter(
        (activeSkill) => activeSkill.skill_id !== skillId
      );
    } catch (caughtError) {
      error.value = formatError(caughtError);
    } finally {
      activatingSkillId.value = null;
    }
  }

  return {
    skills,
    activeSkills,
    selectedSkill,
    loading,
    activatingSkillId,
    error,
    hasSkills,
    activeSkillIds,
    isSkillActive,
    loadSkills,
    loadSkillDetail,
    activateSkill,
    deactivateSkill
  };
});
