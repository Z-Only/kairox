/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- project commands ---- */

registerCommandHandlers({
  list_projects: function (args) {
    return Promise.resolve(state.projects);
  },
  create_blank_project: function (args) {
    var projectId = nextId("prj");
    var displayName = args.displayName || args.display_name || "New Project";
    var project = {
      project_id: projectId,
      display_name: displayName,
      root_path: "/mock/workspace/" + displayName.replace(/\s+/g, "-").toLowerCase(),
      removed_at: null,
      sort_order: state.projects.length,
      expanded: true
    };
    state.projects.push(project);
    state.projectSessions.set(projectId, []);
    state.projectBranches.set(projectId, ["main"]);
    return Promise.resolve(project);
  },
  add_existing_project: function (args) {
    var projectPath = args.path || "/mock/workspace/existing-project";
    var parts = projectPath.split(/[\\/]/).filter(Boolean);
    var projectName = parts.length > 0 ? parts[parts.length - 1] : "Existing Project";
    var existingProjectId = nextId("prj");
    var existingProject = {
      project_id: existingProjectId,
      display_name: projectName,
      root_path: projectPath,
      removed_at: null,
      sort_order: state.projects.length,
      expanded: true
    };
    state.projects.push(existingProject);
    state.projectSessions.set(existingProjectId, []);
    state.projectBranches.set(existingProjectId, ["main"]);
    return Promise.resolve(existingProject);
  },
  remove_project: function (args) {
    var removeProjectId = args.projectId || args.project_id;
    state.projects = state.projects.map(function (project) {
      return project.project_id === removeProjectId
        ? Object.assign({}, project, { removed_at: new Date().toISOString() })
        : project;
    });
    return Promise.resolve(undefined);
  },
  restore_project_session: function (args) {
    var restoreSessionId = args.sessionId || args.session_id;
    var archivedSession = state.archivedSessions.find(function (session) {
      return session.id === restoreSessionId;
    });
    if (!archivedSession) return Promise.reject(new Error("Archived session not found"));
    archivedSession.visibility = "visible";
    state.archivedSessions = state.archivedSessions.filter(function (session) {
      return session.id !== restoreSessionId;
    });
    var restoredProject = getProject(archivedSession.project_id);
    if (!restoredProject) return Promise.reject(new Error("Project not found"));
    var restoredProjectSessions = getProjectSessionList(restoredProject.project_id);
    restoredProjectSessions.push(archivedSession);
    state.projectSessions.set(restoredProject.project_id, restoredProjectSessions);
    return Promise.resolve(restoredProject);
  },
  create_project_draft_session: function (args) {
    var draftProjectId = args.projectId || args.project_id;
    var draftProject = getProject(draftProjectId);
    if (!draftProject) return Promise.reject(new Error("Project not found"));
    var draftSessionId = nextId("ses");
    var draftSession = makeSessionInfo(
      draftSessionId,
      "New conversation",
      "fast",
      draftProjectId,
      draftProject.root_path,
      null,
      "draft_hidden"
    );
    state.sessions.push(draftSession);
    var projectSessions = getProjectSessionList(draftProjectId);
    projectSessions.unshift(draftSession);
    state.projectSessions.set(draftProjectId, projectSessions);
    state.currentSessionId = draftSessionId;
    state.currentProfile = draftSession.profile;
    state.projections.set(draftSessionId, {
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false
    });
    state.traces.set(draftSessionId, []);
    return Promise.resolve(draftSessionId);
  },
  create_project_worktree_session: function (args) {
    var worktreeProjectId = args.projectId || args.project_id;
    var branchName = args.branchName || args.branch_name;
    var worktreeProject = getProject(worktreeProjectId);
    if (!worktreeProject) return Promise.reject(new Error("Project not found"));
    if (!branchName) return Promise.reject(new Error("Branch name is required"));
    var worktreeBranches = state.projectBranches.get(worktreeProjectId) || ["main"];
    if (worktreeBranches.indexOf(branchName) === -1) {
      worktreeBranches = worktreeBranches.concat([branchName]);
      state.projectBranches.set(worktreeProjectId, worktreeBranches);
    }
    var worktreeSessionId = nextId("ses");
    var worktreeSession = makeSessionInfo(
      worktreeSessionId,
      "New Session (" + branchName + ")",
      "fast",
      worktreeProjectId,
      worktreeProject.root_path + "/.kairox/worktrees/" + branchName.replace(/[\\/]/g, "-"),
      branchName,
      "visible"
    );
    state.sessions.push(worktreeSession);
    var worktreeProjectSessions = getProjectSessionList(worktreeProjectId);
    worktreeProjectSessions.unshift(worktreeSession);
    state.projectSessions.set(worktreeProjectId, worktreeProjectSessions);
    state.currentSessionId = worktreeSessionId;
    state.currentProfile = worktreeSession.profile;
    state.projections.set(worktreeSessionId, {
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false
    });
    state.traces.set(worktreeSessionId, []);
    return Promise.resolve(worktreeSessionId);
  },
  list_project_sessions: function (args) {
    var listProjectId = args.projectId || args.project_id;
    return Promise.resolve(
      getProjectSessionList(listProjectId).filter(function (session) {
        return session.visibility !== "archived";
      })
    );
  },
  list_project_branches: function (args) {
    var branchesProjectId = args.projectId || args.project_id;
    var branchesProject = getProject(branchesProjectId);
    if (!branchesProject) return Promise.reject(new Error("Project not found"));
    return Promise.resolve(state.projectBranches.get(branchesProjectId) || ["main"]);
  },
  list_archived_sessions: function (args) {
    return Promise.resolve(state.archivedSessions);
  },
  get_project_git_status: function (args) {
    var statusProjectId = args.projectId || args.project_id;
    var statusProject = getProject(statusProjectId);
    if (!statusProject) return Promise.reject(new Error("Project not found"));
    return Promise.resolve(makeProjectGitStatus(statusProject));
  },
  get_project_git_review: function (args) {
    var reviewProjectId = args.projectId || args.project_id;
    var reviewProject = getProject(reviewProjectId);
    if (!reviewProject) return Promise.reject(new Error("Project not found"));
    return Promise.resolve(makeProjectGitReviewFromStatus(makeProjectGitStatus(reviewProject)));
  },
  get_session_git_status: function (args) {
    var statusSessionId = args.sessionId || args.session_id;
    var statusSession = getSession(statusSessionId);
    if (!statusSession) return Promise.reject(new Error("Session not found"));
    return Promise.resolve({
      kind: "not_initialized",
      branch: statusSession.branch,
      worktree_path: statusSession.worktree_path || "/mock/workspace",
      message: null
    });
  },
  get_session_git_review: function (args) {
    var reviewSessionId = args.sessionId || args.session_id;
    var reviewSession = getSession(reviewSessionId);
    if (!reviewSession) return Promise.reject(new Error("Session not found"));
    return Promise.resolve(
      makeProjectGitReviewFromStatus({
        kind: "not_initialized",
        branch: reviewSession.branch,
        worktree_path: reviewSession.worktree_path || "/mock/workspace",
        message: null
      })
    );
  },
  init_project_git: function (args) {
    var initProjectId = args.projectId || args.project_id;
    var initProject = getProject(initProjectId);
    if (!initProject) return Promise.reject(new Error("Project not found"));
    state.gitStatuses.set(initProjectId, "clean");
    return Promise.resolve(makeProjectGitStatus(initProject));
  }
});
