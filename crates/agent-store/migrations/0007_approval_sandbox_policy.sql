-- Add orthogonal (approval_policy, sandbox_policy) columns alongside the
-- legacy `permission_mode`. Backfill from the existing mode using the same
-- mapping that `From<PermissionMode> for (ApprovalPolicy, SandboxPolicy)`
-- encodes in `agent-tools`. Both columns stay nullable during the transition
-- window so older callers that still write only `permission_mode` continue
-- to work. PR-2e drops the legacy column once all callers are migrated.
ALTER TABLE kairox_sessions ADD COLUMN approval_policy TEXT;
