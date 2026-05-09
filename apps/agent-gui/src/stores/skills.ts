// `unplugin-auto-import` only injects globals into `.vue` SFCs. Pinia stores
// are plain `.ts` modules and must import Vue and Pinia APIs explicitly.
import { computed, ref } from "vue";
import { defineStore } from "pinia";
import {
  commands,
  type ActiveSkillView,
  type SkillDetail,
  type SkillView
} from "@/generated/commands";

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
        commands.listSkills(),
        commands.listActiveSkills()
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
      selectedSkill.value = await commands.getSkillDetail(skillId);
    } catch (caughtError) {
      error.value = formatError(caughtError);
    }
  }

  async function activateSkill(skillId: string): Promise<void> {
    activatingSkillId.value = skillId;
    error.value = null;
    try {
      const activeSkill = await commands.activateSkill(skillId);
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
      await commands.deactivateSkill(skillId);
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
