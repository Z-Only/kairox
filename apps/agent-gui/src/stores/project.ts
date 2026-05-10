import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SessionInfoResponse } from "@/types";

export interface ProjectInfo {
  projectId: string;
  displayName: string;
  rootPath: string;
  removedAt: string | null;
  sortOrder: number;
  expanded: boolean;
}

export interface ProjectSessionInfo {
  sessionId: string;
  title: string;
  profile: string;
  projectId: string | null;
  worktreePath: string | null;
  branch: string | null;
  visibility: string | null;
}

export interface ProjectGitStatusInfo {
  kind: string;
  branch: string | null;
  worktreePath: string;
  message: string | null;
}

export interface ProjectInstructionSummaryInfo {
  sourcePaths: string[];
  warning: string | null;
}

interface ProjectInfoResponse {
  project_id: string;
  display_name: string;
  root_path: string;
  removed_at: string | null;
  sort_order: number;
  expanded: boolean;
}

interface ProjectGitStatusResponse {
  kind: string;
  branch: string | null;
  worktree_path: string;
  message: string | null;
}

interface ProjectInstructionSummaryResponse {
  source_paths: string[];
  warning: string | null;
}

function normalizeProject(response: ProjectInfoResponse): ProjectInfo {
  return {
    projectId: response.project_id,
    displayName: response.display_name,
    rootPath: response.root_path,
    removedAt: response.removed_at,
    sortOrder: response.sort_order,
    expanded: response.expanded
  };
}

function normalizeProjectSession(response: SessionInfoResponse): ProjectSessionInfo {
  return {
    sessionId: response.id,
    title: response.title,
    profile: response.profile,
    projectId: response.project_id,
    worktreePath: response.worktree_path,
    branch: response.branch,
    visibility: response.visibility
  };
}

function normalizeGitStatus(response: ProjectGitStatusResponse): ProjectGitStatusInfo {
  return {
    kind: response.kind,
    branch: response.branch,
    worktreePath: response.worktree_path,
    message: response.message
  };
}

function normalizeInstructionSummary(
  response: ProjectInstructionSummaryResponse
): ProjectInstructionSummaryInfo {
  return {
    sourcePaths: response.source_paths,
    warning: response.warning
  };
}

function upsertProject(projects: ProjectInfo[], project: ProjectInfo): ProjectInfo[] {
  const existingIndex = projects.findIndex((entry) => entry.projectId === project.projectId);
  if (existingIndex === -1) {
    return [...projects, project].sort((first, second) => first.sortOrder - second.sortOrder);
  }

  const nextProjects = [...projects];
  nextProjects[existingIndex] = project;
  return nextProjects.sort((first, second) => first.sortOrder - second.sortOrder);
}

function createDraftSessionPlaceholder(
  sessionId: string,
  project: ProjectInfo | undefined
): ProjectSessionInfo {
  return {
    sessionId,
    title: "New conversation",
    profile: "default",
    projectId: project?.projectId ?? null,
    worktreePath: project?.rootPath ?? null,
    branch: null,
    visibility: "draft_hidden"
  };
}

export const useProjectStore = defineStore("project", () => {
  const projects = ref<ProjectInfo[]>([]);
  const sessionsByProject = ref(new Map<string, ProjectSessionInfo[]>());
  const archivedSessions = ref<ProjectSessionInfo[]>([]);

  const activeProjects = computed(() => projects.value.filter((project) => !project.removedAt));

  async function loadProjects(): Promise<void> {
    const responses = await invoke<ProjectInfoResponse[]>("list_projects");
    projects.value = responses.map(normalizeProject);
  }

  async function createBlankProject(displayName?: string): Promise<ProjectInfo> {
    const response = await invoke<ProjectInfoResponse>("create_blank_project", {
      displayName: displayName ?? null
    });
    const project = normalizeProject(response);
    projects.value = upsertProject(projects.value, project);
    return project;
  }

  async function addExistingProject(path: string): Promise<ProjectInfo> {
    const response = await invoke<ProjectInfoResponse>("add_existing_project", { path });
    const project = normalizeProject(response);
    projects.value = upsertProject(projects.value, project);
    return project;
  }

  async function renameProject(projectId: string, displayName: string): Promise<void> {
    await invoke("rename_project", { projectId, displayName });
    projects.value = projects.value.map((project) =>
      project.projectId === projectId ? { ...project, displayName } : project
    );
  }

  async function removeProject(projectId: string): Promise<void> {
    await invoke("remove_project", { projectId });
    await loadProjects();
  }

  async function restoreProjectSession(sessionId: string): Promise<ProjectInfo> {
    const response = await invoke<ProjectInfoResponse>("restore_project_session", { sessionId });
    const project = normalizeProject(response);
    projects.value = upsertProject(projects.value, project);
    await loadProjectSessions(project.projectId);
    return project;
  }

  async function updateProjectOrder(projectIds: string[]): Promise<void> {
    await invoke("update_project_order", { projectIds });
    const orderByProjectId = new Map(
      projectIds.map((projectId, sortOrder) => [projectId, sortOrder] as const)
    );
    projects.value = projects.value
      .map((project) => ({
        ...project,
        sortOrder: orderByProjectId.get(project.projectId) ?? project.sortOrder
      }))
      .sort((first, second) => first.sortOrder - second.sortOrder);
  }

  async function updateProjectExpanded(projectId: string, expanded: boolean): Promise<void> {
    await invoke("update_project_expanded", { projectId, expanded });
    projects.value = projects.value.map((project) =>
      project.projectId === projectId ? { ...project, expanded } : project
    );
  }

  async function createProjectDraftSession(projectId: string): Promise<ProjectSessionInfo> {
    const sessionId = await invoke<string>("create_project_draft_session", { projectId });
    const project = projects.value.find((entry) => entry.projectId === projectId);
    const draftSession = createDraftSessionPlaceholder(sessionId, project);
    const currentSessions = sessionsByProject.value.get(projectId) ?? [];
    const nextSessionsByProject = new Map(sessionsByProject.value);
    nextSessionsByProject.set(projectId, [
      draftSession,
      ...currentSessions.filter((session) => session.sessionId !== sessionId)
    ]);
    sessionsByProject.value = nextSessionsByProject;
    return draftSession;
  }

  async function loadProjectSessions(projectId: string): Promise<void> {
    const responses = await invoke<SessionInfoResponse[]>("list_project_sessions", { projectId });
    const nextSessionsByProject = new Map(sessionsByProject.value);
    nextSessionsByProject.set(projectId, responses.map(normalizeProjectSession));
    sessionsByProject.value = nextSessionsByProject;
  }

  async function loadArchivedSessions(): Promise<void> {
    const responses = await invoke<SessionInfoResponse[]>("list_archived_sessions");
    archivedSessions.value = responses.map(normalizeProjectSession);
  }

  async function getProjectGitStatus(projectId: string): Promise<ProjectGitStatusInfo> {
    const response = await invoke<ProjectGitStatusResponse>("get_project_git_status", {
      projectId
    });
    return normalizeGitStatus(response);
  }

  async function getSessionGitStatus(sessionId: string): Promise<ProjectGitStatusInfo> {
    const response = await invoke<ProjectGitStatusResponse>("get_session_git_status", {
      sessionId
    });
    return normalizeGitStatus(response);
  }

  async function initProjectGit(projectId: string): Promise<ProjectGitStatusInfo> {
    const response = await invoke<ProjectGitStatusResponse>("init_project_git", { projectId });
    return normalizeGitStatus(response);
  }

  async function getProjectInstructionSummary(
    projectId: string
  ): Promise<ProjectInstructionSummaryInfo> {
    const response = await invoke<ProjectInstructionSummaryResponse>(
      "get_project_instruction_summary",
      { projectId }
    );
    return normalizeInstructionSummary(response);
  }

  return {
    projects,
    sessionsByProject,
    archivedSessions,
    activeProjects,
    loadProjects,
    createBlankProject,
    addExistingProject,
    renameProject,
    removeProject,
    restoreProjectSession,
    updateProjectOrder,
    updateProjectExpanded,
    createProjectDraftSession,
    loadProjectSessions,
    loadArchivedSessions,
    getProjectGitStatus,
    getSessionGitStatus,
    initProjectGit,
    getProjectInstructionSummary
  };
});
