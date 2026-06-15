import { defineStore } from "pinia";
import { ref } from "vue";
import { useProjectStore, type ProjectGitReviewInfo } from "@/stores/project";

export type SidebarSection = "projects" | "sessions";
export type RightPanelTab = "trace" | "tasks" | "memory" | "changes" | "trajectory";

export interface GitReviewContext {
  sessionId?: string | null;
  projectId?: string | null;
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function shouldFallbackToProjectGitReview(error: unknown): boolean {
  return errorMessage(error).includes("session is not bound to a project");
}

export const useWorkspaceUiStore = defineStore("workspaceUi", () => {
  const sectionOrder = ref<SidebarSection[]>(["projects", "sessions"]);
  const archiveOpen = ref(false);
  const rightPanelTab = ref<RightPanelTab>("trace");
  const gitReviewContext = ref<GitReviewContext | null>(null);
  const gitReview = ref<ProjectGitReviewInfo | null>(null);
  const gitReviewLoading = ref(false);
  const gitReviewError = ref<string | null>(null);

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

  function setRightPanelTab(tab: RightPanelTab): void {
    rightPanelTab.value = tab;
  }

  function clearGitReview(): void {
    gitReviewContext.value = null;
    gitReview.value = null;
    gitReviewError.value = null;
    gitReviewLoading.value = false;
  }

  async function refreshGitReview(): Promise<void> {
    const context = gitReviewContext.value;
    if (!context?.sessionId && !context?.projectId) return;

    const projectStore = useProjectStore();
    gitReviewLoading.value = true;
    gitReviewError.value = null;
    gitReview.value = null;
    try {
      if (context.sessionId) {
        try {
          gitReview.value = await projectStore.getSessionGitReview(context.sessionId);
        } catch (error) {
          if (!context.projectId || !shouldFallbackToProjectGitReview(error)) throw error;
          gitReview.value = await projectStore.getProjectGitReview(context.projectId);
        }
      } else if (context.projectId) {
        gitReview.value = await projectStore.getProjectGitReview(context.projectId);
      }
    } catch (error) {
      gitReviewError.value = errorMessage(error);
    } finally {
      gitReviewLoading.value = false;
    }
  }

  async function openGitReview(context: GitReviewContext): Promise<void> {
    gitReviewContext.value = context;
    rightPanelTab.value = "changes";
    await refreshGitReview();
  }

  return {
    sectionOrder,
    archiveOpen,
    rightPanelTab,
    gitReviewContext,
    gitReview,
    gitReviewLoading,
    gitReviewError,
    moveSectionUp,
    setRightPanelTab,
    clearGitReview,
    refreshGitReview,
    openGitReview
  };
});
