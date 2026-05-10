CREATE TABLE IF NOT EXISTS kairox_projects (
  project_id TEXT PRIMARY KEY,
  workspace_id TEXT NOT NULL,
  display_name TEXT NOT NULL,
  root_path TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  removed_at TEXT,
  sort_order INTEGER NOT NULL DEFAULT 0,
  expanded INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS kairox_project_sessions (
  session_id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL,
  worktree_path TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS kairox_session_visibility (
  session_id TEXT PRIMARY KEY,
  visibility TEXT NOT NULL DEFAULT 'visible',
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS kairox_workspace_sidebar_prefs (
  workspace_id TEXT PRIMARY KEY,
  section_order_json TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_kairox_projects_workspace_active
  ON kairox_projects(workspace_id, removed_at, sort_order);

CREATE INDEX IF NOT EXISTS idx_kairox_project_sessions_project
  ON kairox_project_sessions(project_id);
