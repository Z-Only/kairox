-- Add the approval axis for per-session tool policy.
ALTER TABLE kairox_sessions ADD COLUMN approval_policy TEXT;
