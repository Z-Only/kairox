/**
 * Browser-side Tauri mock fragment for Playwright E2E tests.
 *
 * These files are concatenated by e2e/helpers/tauriMock.ts and injected with
 * page.addInitScript before app code runs. Keep them plain JavaScript.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

/* ---- workspace commands ---- */

registerCommandHandlers({
  initialize_workspace: function (args) {
    if (state.initialized) return Promise.reject(new Error("Workspace already initialized"));
    var ws = { workspace_id: "wrk_mock", path: "/mock/workspace" };
    state.workspace = ws;
    state.initialized = true;
    // Auto-create a first session
    var sid = nextId("ses");
    var session = makeSessionInfo(sid, "Session using fast", "fast", null, null, null, "visible");
    state.sessions.push(session);
    state.currentSessionId = sid;
    state.projections.set(sid, {
      messages: [],
      task_titles: [],
      task_graph: { tasks: [] },
      token_stream: "",
      cancelled: false
    });
    state.traces.set(sid, []);
    return Promise.resolve(ws);
  },
  list_workspaces: function (args) {
    return Promise.resolve(state.workspace ? [state.workspace] : []);
  },
  restore_workspace: function (args) {
    var _workspaceId = args.workspaceId || args.workspace_id;
    if (state.sessions.length > 0) {
      state.currentSessionId = state.sessions[0].id;
    }
    return Promise.resolve(undefined);
  },
  get_project_instruction_summary: function (args) {
    return Promise.resolve({ source_paths: [], warning: null });
  },
  get_build_info: function (args) {
    return Promise.resolve({
      version: "0.12.0-e2e",
      git_hash: "mock",
      build_time: "2026-05-05"
    });
  },
  "plugin:dialog|open": function (args) {
    var selected = state.nextOpenDialogResult;
    state.nextOpenDialogResult = null;
    return Promise.resolve(selected);
  },
  open_config_dir: function (args) {
    return "/mock/path/to/config";
  },
  list_workspace_files: function (args) {
    return Promise.resolve({ paths: state.workspaceFiles.slice() });
  },
  save_draft: function (args) {
    state.drafts.set(args.request.session_id, args.request.draft_text);
    return Promise.resolve(undefined);
  },
  get_draft: function (args) {
    var draftSessionId = args.sessionId || args.session_id;
    return Promise.resolve(state.drafts.get(draftSessionId) || "");
  }
});
