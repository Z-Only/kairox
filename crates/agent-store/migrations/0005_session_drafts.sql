CREATE TABLE IF NOT EXISTS session_drafts (
    session_id TEXT PRIMARY KEY,
    draft_text TEXT NOT NULL DEFAULT '',
    updated_at TEXT NOT NULL,
    FOREIGN KEY (session_id) REFERENCES kairox_sessions(session_id)
);
