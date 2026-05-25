/**
 * Browser-side Tauri mock fragment — project fixtures.
 */

// @ts-nocheck
/* eslint-disable no-unused-vars */

state.projects = [
  {
    project_id: "prj_mock",
    display_name: "Mock Project",
    root_path: "/mock/workspace",
    removed_at: null,
    sort_order: 0,
    expanded: true
  }
];
state.projectSessions = new Map();
state.archivedSessions = [];
state.gitStatuses = new Map();
state.projectBranches = new Map([["prj_mock", ["main", "develop"]]]);
