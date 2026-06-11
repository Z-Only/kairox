import { defineStore } from "pinia";
import { computed, ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { commands } from "@/generated/commands";
import type { SessionInfoResponse } from "@/types";
import { useSessionStore, uniqueSessionTitle } from "@/stores/session";

export interface ProjectInfo {
  projectId: string;
  displayName: string;
  rootPath: string;
  removedAt: string | null;
  sortOrder: number;
  expanded: boolean;
  pathExists: boolean;
}

export interface ProjectSessionInfo {
  sessionId: string;
  title: string;
  profile: string;
  projectId: string | null;
  worktreePath: string | null;
  branch: string | null;
  visibility: string | null;
  deletedAt: string | null;
  approvalPolicy: string | null;
  sandboxPolicy: string | null;
}

export interface ProjectGitStatusInfo {
  kind: string;
  branch: string | null;
  worktreePath: string;
  message: string | null;
}

export interface ProjectGitDiffSectionInfo {
  label: string;
  stat: string;
  diff: string;
}

export interface ProjectGitReviewInfo extends ProjectGitStatusInfo {
  changedFiles: string[];
  staged: ProjectGitDiffSectionInfo | null;
  unstaged: ProjectGitDiffSectionInfo | null;
  untracked: ProjectGitDiffSectionInfo | null;
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
  path_exists: boolean;
}

interface ProjectGitStatusResponse {
  kind: string;
  branch: string | null;
  worktree_path: string;
  message: string | null;
}

interface ProjectGitDiffSectionResponse {
  label: string;
  stat: string;
  diff: string;
}

interface ProjectGitReviewResponse extends ProjectGitStatusResponse {
  changed_files: string[];
  staged: ProjectGitDiffSectionResponse | null;
  unstaged: ProjectGitDiffSectionResponse | null;
  untracked: ProjectGitDiffSectionResponse | null;
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
    expanded: response.expanded,
    pathExists: response.path_exists
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
    visibility: response.visibility,
    deletedAt: response.deleted_at,
    approvalPolicy: response.approval_policy,
    sandboxPolicy: response.sandbox_policy
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

function normalizeGitDiffSection(
  section: ProjectGitDiffSectionResponse | null
): ProjectGitDiffSectionInfo | null {
  if (!section) return null;
  return {
    label: section.label,
    stat: section.stat,
    diff: section.diff
  };
}

function normalizeGitReview(response: ProjectGitReviewResponse): ProjectGitReviewInfo {
  return {
    ...normalizeGitStatus(response),
    changedFiles: response.changed_files,
    staged: normalizeGitDiffSection(response.staged),
    unstaged: normalizeGitDiffSection(response.unstaged),
    untracked: normalizeGitDiffSection(response.untracked)
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
  project: ProjectInfo | undefined,
  branch?: string | null
): ProjectSessionInfo {
  return {
    sessionId,
    title: "New Session",
    profile: "default",
    projectId: project?.projectId ?? null,
    worktreePath: project?.rootPath ?? null,
    branch: branch ?? null,
    visibility: "draft_hidden",
    deletedAt: null,
    approvalPolicy: null,
    sandboxPolicy: null
  };
}

async function refreshConfigForProject(rootPath: string): Promise<void> {
  try {
    await commands.refreshConfigForProject(rootPath);
    const sessionStore = useSessionStore();
    await sessionStore.loadProfileInfo({ force: true });
  } catch (error) {
    console.error("Failed to refresh config for project:", error);
  }
}

export const useProjectStore = defineStore("project", () => {
  const projects = ref<ProjectInfo[]>([]);
  const sessionsByProject = ref(new Map<string, ProjectSessionInfo[]>());
  const archivedSessions = ref<ProjectSessionInfo[]>([]);
  const instructionSummariesByProject = ref(new Map<string, ProjectInstructionSummaryInfo>());

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

  async function refreshProjectConfig(projectId: string): Promise<void> {
    const project = projects.value.find((entry) => entry.projectId === projectId);
    if (project?.rootPath) {
      await refreshConfigForProject(project.rootPath);
    }
  }

  async function refreshProjectConfigRoot(rootPath: string): Promise<void> {
    await refreshConfigForProject(rootPath);
  }

  async function createProjectWorktreeSession(
    projectId: string,
    branchName: string
  ): Promise<ProjectSessionInfo> {
    const project = projects.value.find((entry) => entry.projectId === projectId);
    if (project?.rootPath) {
      await refreshConfigForProject(project.rootPath);
    }
    const sessionId = await invoke<string>("create_project_worktree_session", {
      projectId,
      branchName
    });
    let worktreePath: string | null = null;
    let branch: string | null = branchName;
    try {
      const gitStatus = await getSessionGitStatus(sessionId);
      worktreePath = gitStatus.worktreePath;
      branch = gitStatus.branch ?? branchName;
    } catch {
      // Session creation already succeeded; git status is only display metadata.
    }
    const draftSession: ProjectSessionInfo = {
      sessionId,
      title: `New Session (${branchName})`,
      profile: "default",
      projectId: project?.projectId ?? null,
      worktreePath,
      branch,
      visibility: "visible",
      deletedAt: null,
      approvalPolicy: null,
      sandboxPolicy: null
    };

    const projectSessions = sessionsByProject.value.get(projectId) ?? [];
    const existingTitles = projectSessions.map((s) => s.title);
    draftSession.title = uniqueSessionTitle(`New Session (${branchName})`, existingTitles);

    try {
      await invoke("rename_session", {
        sessionId: draftSession.sessionId,
        title: draftSession.title
      });
    } catch (e) {
      console.error("Failed to set deduped worktree session title:", e);
    }

    const nextSessionsByProject = new Map(sessionsByProject.value);
    nextSessionsByProject.set(projectId, [
      draftSession,
      ...projectSessions.filter((session) => session.sessionId !== sessionId)
    ]);
    sessionsByProject.value = nextSessionsByProject;
    return draftSession;
  }

  async function listProjectBranches(projectId: string): Promise<string[]> {
    return await invoke<string[]>("list_project_branches", { projectId });
  }

  async function createProjectDraftSession(projectId: string): Promise<ProjectSessionInfo> {
    const project = projects.value.find((entry) => entry.projectId === projectId);
    if (project?.rootPath) {
      await refreshConfigForProject(project.rootPath);
    }
    const sessionId = await invoke<string>("create_project_draft_session", { projectId });
    let branch: string | null = null;
    if (project?.rootPath) {
      try {
        const gitStatus = await getProjectGitStatus(projectId);
        branch = gitStatus.branch;
      } catch {
        // git status may fail — non-critical
      }
    }
    const draftSession = createDraftSessionPlaceholder(sessionId, project, branch);

    // Dedup within this project's sessions
    const projectSessions = sessionsByProject.value.get(projectId) ?? [];
    const existingTitles = projectSessions.map((s) => s.title);
    draftSession.title = uniqueSessionTitle("New Session", existingTitles);

    // Persist the deduped title
    try {
      await invoke("rename_session", {
        sessionId: draftSession.sessionId,
        title: draftSession.title
      });
    } catch (e) {
      console.error("Failed to set deduped project session title:", e);
    }

    const nextSessionsByProject = new Map(sessionsByProject.value);
    nextSessionsByProject.set(projectId, [
      draftSession,
      ...projectSessions.filter((session) => session.sessionId !== sessionId)
    ]);
    sessionsByProject.value = nextSessionsByProject;
    return draftSession;
  }

  async function loadProjectSessions(projectId: string): Promise<void> {
    const project = projects.value.find((entry) => entry.projectId === projectId);
    if (project?.rootPath) {
      await refreshConfigForProject(project.rootPath);
    }
    const responses = await invoke<SessionInfoResponse[]>("list_project_sessions", { projectId });
    const nextSessionsByProject = new Map(sessionsByProject.value);
    nextSessionsByProject.set(projectId, responses.map(normalizeProjectSession));
    sessionsByProject.value = nextSessionsByProject;
  }

  async function loadArchivedSessions(): Promise<void> {
    const responses = await invoke<SessionInfoResponse[]>("list_archived_sessions");
    archivedSessions.value = responses.map(normalizeProjectSession);
  }

  async function renameProjectSession(sessionId: string, title: string): Promise<void> {
    await invoke("rename_session", { sessionId, title });
    const nextSessionsByProject = new Map(sessionsByProject.value);

    for (const [projectId, projectSessions] of nextSessionsByProject.entries()) {
      nextSessionsByProject.set(
        projectId,
        projectSessions.map((projectSession) =>
          projectSession.sessionId === sessionId ? { ...projectSession, title } : projectSession
        )
      );
    }

    sessionsByProject.value = nextSessionsByProject;
  }

  async function archiveProjectSession(sessionId: string): Promise<void> {
    await invoke("delete_session", { sessionId });
    const nextSessionsByProject = new Map(sessionsByProject.value);

    for (const [projectId, projectSessions] of nextSessionsByProject.entries()) {
      nextSessionsByProject.set(
        projectId,
        projectSessions.filter((projectSession) => projectSession.sessionId !== sessionId)
      );
    }

    sessionsByProject.value = nextSessionsByProject;
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

  async function getProjectGitReview(projectId: string): Promise<ProjectGitReviewInfo> {
    const response = await invoke<ProjectGitReviewResponse>("get_project_git_review", {
      projectId
    });
    return normalizeGitReview(response);
  }

  async function getSessionGitReview(sessionId: string): Promise<ProjectGitReviewInfo> {
    const response = await invoke<ProjectGitReviewResponse>("get_session_git_review", {
      sessionId
    });
    return normalizeGitReview(response);
  }

  async function initProjectGit(projectId: string): Promise<ProjectGitStatusInfo> {
    const response = await invoke<ProjectGitStatusResponse>("init_project_git", { projectId });
    return normalizeGitStatus(response);
  }

  async function getProjectInstructionSummary(
    projectId: string
  ): Promise<ProjectInstructionSummaryInfo> {
    let instructionSummary: ProjectInstructionSummaryInfo;
    try {
      const response = await invoke<ProjectInstructionSummaryResponse>(
        "get_project_instruction_summary",
        { projectId }
      );
      instructionSummary = normalizeInstructionSummary(response);
    } catch (error) {
      instructionSummary = {
        sourcePaths: [],
        warning: String(error)
      };
    }

    const nextInstructionSummaries = new Map(instructionSummariesByProject.value);
    nextInstructionSummaries.set(projectId, instructionSummary);
    instructionSummariesByProject.value = nextInstructionSummaries;
    return instructionSummary;
  }

  return {
    projects,
    sessionsByProject,
    archivedSessions,
    instructionSummariesByProject,
    activeProjects,
    loadProjects,
    createBlankProject,
    addExistingProject,
    renameProject,
    removeProject,
    restoreProjectSession,
    updateProjectOrder,
    updateProjectExpanded,
    refreshProjectConfig,
    refreshProjectConfigRoot,
    createProjectWorktreeSession,
    listProjectBranches,
    createProjectDraftSession,
    loadProjectSessions,
    loadArchivedSessions,
    renameProjectSession,
    archiveProjectSession,
    getProjectGitStatus,
    getSessionGitStatus,
    getProjectGitReview,
    getSessionGitReview,
    initProjectGit,
    getProjectInstructionSummary
  };
});
