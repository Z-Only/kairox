CREATE TABLE IF NOT EXISTS kairox_workspaces (
    workspace_id  TEXT PRIMARY KEY,
    path          TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS kairox_sessions (
    session_id    TEXT PRIMARY KEY,
    workspace_id  TEXT NOT NULL REFERENCES kairox_workspaces(workspace_id),
    title         TEXT NOT NULL,
    model_profile TEXT NOT NULL,
    model_id      TEXT,
    provider      TEXT,
    deleted_at    TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_sessions_workspace ON kairox_sessions(workspace_id);
CREATE INDEX IF NOT EXISTS idx_sessions_deleted ON kairox_sessions(deleted_at);
