import { open } from "@tauri-apps/plugin-dialog";
import { computed, ref } from "vue";
import { useRoute, useRouter } from "vue-router";
import { useProjectStore, type ProjectInfo, type ProjectSessionInfo } from "@/stores/project";
import { useSessionStore } from "@/stores/session";

export function useSidebarActions() {
  const session = useSessionStore();
  const projects = useProjectStore();
  const route = useRoute();
  const router = useRouter();

  const activeSessionId = computed<string | null>(() => {
    const v = route.params.sessionId;
    const id = Array.isArray(v) ? v[0] : v;
    return id ?? session.currentSessionId;
  });

  const projectCreateMenuOpen = ref(false);
  const pendingDeleteSessionId = ref<string | null>(null);
  const pendingDeleteProjectId = ref<string | null>(null);
  const pendingArchiveProjectSessionId = ref<string | null>(null);
  const importingProject = ref(false);

  function resetDeleteConfirmation() {
    pendingDeleteSessionId.value = null;
    pendingDeleteProjectId.value = null;
    pendingArchiveProjectSessionId.value = null;
  }

  async function switchToSession(sessionId: string) {
    if (sessionId === activeSessionId.value) return;
    resetDeleteConfirmation();
    try {
      await router.push({ name: "workbench", params: { sessionId } });
    } catch (e) {
      console.error("Failed to navigate to session:", e);
    }
  }

  async function createSession() {
    resetDeleteConfirmation();
    try {
      await session.startOrdinaryDraftSession();
      await router.push({ name: "workbench" });
    } catch (e) {
      console.error("Failed to start session:", e);
    }
  }

  async function requestDeleteSession(sessionId: string) {
    if (pendingDeleteSessionId.value !== sessionId) {
      pendingDeleteSessionId.value = sessionId;
      pendingDeleteProjectId.value = null;
      return;
    }
    await session.deleteSession(sessionId);
    pendingDeleteSessionId.value = null;
  }

  function getProjectSessions(projectId: string): ProjectSessionInfo[] {
    return projects.sessionsByProject.get(projectId) ?? [];
  }

  async function activateProjectSession(projectSession: ProjectSessionInfo) {
    resetDeleteConfirmation();
    await session.switchProjectSession(projectSession);
    await router.push({ name: "workbench", params: { sessionId: projectSession.sessionId } });
  }

  async function switchToProjectSession(projectSession: ProjectSessionInfo) {
    try {
      await activateProjectSession(projectSession);
    } catch (e) {
      console.error("Failed to open project session:", e);
    }
  }

  async function createProjectSession(projectId: string) {
    try {
      resetDeleteConfirmation();
      await session.startProjectDraftSession(projectId);
      await router.push({ name: "workbench" });
    } catch (e) {
      console.error("Failed to start project session:", e);
    }
  }

  async function createBlankProject() {
    resetDeleteConfirmation();
    try {
      await projects.createBlankProject();
      projectCreateMenuOpen.value = false;
    } catch (e) {
      console.error("Failed to create blank project:", e);
    }
  }

  async function importExistingProject() {
    if (importingProject.value) return;
    resetDeleteConfirmation();
    importingProject.value = true;
    try {
      const selectedPath = await open({ directory: true, multiple: false });
      if (!selectedPath || Array.isArray(selectedPath)) return;
      await projects.addExistingProject(selectedPath);
      await projects.loadProjects();
      projectCreateMenuOpen.value = false;
    } finally {
      importingProject.value = false;
    }
  }

  async function requestArchiveProjectSession(sessionId: string) {
    if (pendingArchiveProjectSessionId.value !== sessionId) {
      pendingArchiveProjectSessionId.value = sessionId;
      pendingDeleteSessionId.value = null;
      pendingDeleteProjectId.value = null;
      return;
    }
    try {
      await projects.archiveProjectSession(sessionId);
    } catch (e) {
      console.error("Failed to archive project session:", e);
    }
    pendingArchiveProjectSessionId.value = null;
  }

  async function toggleProjectExpanded(project: ProjectInfo) {
    const expanded = !project.expanded;
    try {
      await projects.updateProjectExpanded(project.projectId, expanded);
      if (expanded) {
        await projects.loadProjectSessions(project.projectId);
      }
    } catch (e) {
      console.error("Failed to update project expansion:", e);
    }
  }

  async function requestDeleteProject(projectId: string) {
    if (pendingDeleteProjectId.value !== projectId) {
      pendingDeleteProjectId.value = projectId;
      pendingDeleteSessionId.value = null;
      return;
    }
    await projects.removeProject(projectId);
    pendingDeleteProjectId.value = null;
  }

  async function loadProjectsForSidebar() {
    try {
      await projects.loadProjects();
      await Promise.all(
        projects.activeProjects
          .filter((project) => project.expanded)
          .map((project) => projects.loadProjectSessions(project.projectId))
      );
    } catch (e) {
      console.error("Failed to load projects:", e);
    }
  }

  return {
    activeSessionId,
    projectCreateMenuOpen,
    pendingDeleteSessionId,
    pendingDeleteProjectId,
    pendingArchiveProjectSessionId,
    importingProject,
    resetDeleteConfirmation,
    switchToSession,
    createSession,
    requestDeleteSession,
    getProjectSessions,
    switchToProjectSession,
    createProjectSession,
    createBlankProject,
    importExistingProject,
    requestArchiveProjectSession,
    toggleProjectExpanded,
    requestDeleteProject,
    loadProjectsForSidebar
  };
}
