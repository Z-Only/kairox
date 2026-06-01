CREATE TABLE IF NOT EXISTS trajectories (
    trajectory_id TEXT PRIMARY KEY,
    task_id       TEXT NOT NULL,
    session_id    TEXT NOT NULL,
    started_at    TEXT NOT NULL,
    completed_at  TEXT,
    outcome       TEXT NOT NULL DEFAULT 'in_progress'
);

CREATE INDEX IF NOT EXISTS idx_trajectories_session ON trajectories(session_id);

CREATE TABLE IF NOT EXISTS trajectory_steps (
    trajectory_id TEXT NOT NULL,
    step_index    INTEGER NOT NULL,
    action        TEXT NOT NULL,
    action_input  TEXT NOT NULL,
    observation   TEXT NOT NULL,
    screenshot_id TEXT,
    timestamp     TEXT NOT NULL,
    duration_ms   INTEGER NOT NULL,
    PRIMARY KEY (trajectory_id, step_index),
    FOREIGN KEY (trajectory_id) REFERENCES trajectories(trajectory_id) ON DELETE CASCADE
);
