CREATE TABLE IF NOT EXISTS kairox_autonomous_tasks (
    autonomous_task_id TEXT PRIMARY KEY,
    workspace_id TEXT NOT NULL,
    goal_json TEXT NOT NULL,
    config_json TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'active',
    current_session_id TEXT,
    session_count INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS kairox_autonomous_checkpoints (
    checkpoint_id TEXT PRIMARY KEY,
    autonomous_task_id TEXT NOT NULL REFERENCES kairox_autonomous_tasks(autonomous_task_id),
    session_id TEXT NOT NULL,
    session_index INTEGER NOT NULL,
    checkpoint_json TEXT NOT NULL,
    end_reason TEXT NOT NULL,
    created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_autonomous_checkpoints_task
    ON kairox_autonomous_checkpoints(autonomous_task_id);

CREATE TABLE IF NOT EXISTS kairox_autonomous_session_chain (
    autonomous_task_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    session_index INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    PRIMARY KEY (autonomous_task_id, session_index)
);

CREATE INDEX IF NOT EXISTS idx_autonomous_chain_task
    ON kairox_autonomous_session_chain(autonomous_task_id);
CREATE INDEX IF NOT EXISTS idx_autonomous_chain_session
    ON kairox_autonomous_session_chain(session_id);
