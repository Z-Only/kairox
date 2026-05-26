UPDATE kairox_sessions
SET approval_policy = CASE permission_mode
        WHEN 'read_only'  THEN 'never'
        WHEN 'suggest'    THEN 'always'
        WHEN 'agent'      THEN 'on_request'
        WHEN 'interactive' THEN 'on_request'
        WHEN 'autonomous' THEN 'never'
        ELSE 'on_request'
    END,
    sandbox_policy = CASE permission_mode
        WHEN 'read_only'  THEN '{"kind":"read_only"}'
        WHEN 'suggest'    THEN '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
        WHEN 'agent'      THEN '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
        WHEN 'interactive' THEN '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
        WHEN 'autonomous' THEN '{"kind":"danger_full_access"}'
        ELSE '{"kind":"workspace_write","network_access":false,"writable_roots":[]}'
    END
WHERE approval_policy IS NULL OR sandbox_policy IS NULL;
