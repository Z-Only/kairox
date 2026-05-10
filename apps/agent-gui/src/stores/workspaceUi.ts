import { defineStore } from "pinia";
import { ref } from "vue";

export type SidebarSection = "projects" | "sessions";

export const useWorkspaceUiStore = defineStore("workspaceUi", () => {
  const sectionOrder = ref<SidebarSection[]>(["projects", "sessions"]);
  const archiveOpen = ref(false);

  function moveSectionUp(section: SidebarSection): void {
    const index = sectionOrder.value.indexOf(section);
    if (index <= 0) return;

    const nextSectionOrder = [...sectionOrder.value];
    [nextSectionOrder[index - 1], nextSectionOrder[index]] = [
      nextSectionOrder[index],
      nextSectionOrder[index - 1]
    ];
    sectionOrder.value = nextSectionOrder;
  }

  return {
    sectionOrder,
    archiveOpen,
    moveSectionUp
  };
});
