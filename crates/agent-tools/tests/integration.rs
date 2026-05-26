use agent_tools::permission::{PermissionEngine, PermissionOutcome, ToolRisk};
use agent_tools::policy::{ApprovalPolicy, SandboxPolicy};
use agent_tools::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput, ToolRegistry};
use agent_tools::{FsListTool, FsReadTool, FsWriteTool, ShellExecTool};

fn ws_default() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        network_access: false,
        writable_roots: vec![],
    }
}
use async_trait::async_trait;
use std::path::PathBuf;

// ── Helpers ──────────────────────────────────────────────────────────────────

/// Create a `ToolInvocation` with reasonable defaults for every required field.
fn invocation(tool_id: &str, arguments: serde_json::Value) -> ToolInvocation {
    ToolInvocation {
        tool_id: tool_id.into(),
        arguments,
        workspace_id: "test-ws".into(),
        preview: format!("{tool_id}()"),
        timeout_ms: 5_000,
        output_limit_bytes: 102_400,
    }
}

// ── Custom EchoTool ──────────────────────────────────────────────────────────

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "echo".into(),
            description: "Echoes input back to the caller".into(),
            required_capability: "echo".into(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }

    fn risk(&self, _inv: &ToolInvocation) -> ToolRisk {
        ToolRisk::read("echo")
    }

    async fn invoke(&self, _inv: ToolInvocation) -> agent_tools::Result<ToolOutput> {
        Ok(ToolOutput {
            text: "echo ok".into(),
            truncated: false,
        })
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 1 — registry lists all tools including custom
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn registry_lists_all_tools_including_custom() {
    let mut registry = ToolRegistry::new();

    // Register a custom EchoTool
    registry.register(Box::new(EchoTool));

    // list_all() is async — should include the custom tool
    let all = registry.list_all().await;
    assert!(
        all.iter().any(|d| d.tool_id == "echo"),
        "list_all() should include the registered 'echo' tool"
    );

    // Verify the tool can be retrieved via get()
    let tool = registry.get("echo").await;
    assert!(tool.is_some(), "get('echo') should return the EchoTool");
    assert!(
        registry.get("nonexistent").await.is_none(),
        "get('nonexistent') should return None"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 2 — PermissionEngine decide() per (approval, sandbox) pair
// ═══════════════════════════════════════════════════════════════════════════════

#[test]
fn permission_engine_decide_per_policy_pair() {
    // ── (Never, ReadOnly) ───────────────────────────────────────────────────
    let engine = PermissionEngine::new(ApprovalPolicy::Never, SandboxPolicy::ReadOnly);

    // fs.read (read) → Allowed
    assert!(
        matches!(
            engine.decide(&ToolRisk::read("fs.read")),
            PermissionOutcome::Allowed
        ),
        "ReadOnly should allow reads"
    );

    // shell.exec (non-destructive) → Denied
    let shell_decision = engine.decide(&ToolRisk::shell("shell.exec", false));
    assert!(
        matches!(shell_decision, PermissionOutcome::Denied(_)),
        "ReadOnly should deny shell execution, got: {shell_decision:?}"
    );

    // fs.write (write) → Denied
    let write_decision = engine.decide(&ToolRisk::write("fs.write"));
    assert!(
        matches!(write_decision, PermissionOutcome::Denied(_)),
        "ReadOnly should deny writes, got: {write_decision:?}"
    );

    // destructive → Denied
    let destroy_decision = engine.decide(&ToolRisk::destructive("rm.rf"));
    assert!(
        matches!(destroy_decision, PermissionOutcome::Denied(_)),
        "ReadOnly should deny destructive ops, got: {destroy_decision:?}"
    );

    // ── (Always, WorkspaceWrite) ────────────────────────────────────────────
    let always = PermissionEngine::new(ApprovalPolicy::Always, ws_default());
    assert!(
        matches!(
            always.decide(&ToolRisk::read("fs.read")),
            PermissionOutcome::Allowed
        ),
        "Always approval should allow reads"
    );
    assert!(
        matches!(
            always.decide(&ToolRisk::write("fs.write")),
            PermissionOutcome::RequiresApproval
        ),
        "Always approval should require approval for writes"
    );
    assert!(
        matches!(
            always.decide(&ToolRisk::shell("shell.exec", false)),
            PermissionOutcome::RequiresApproval
        ),
        "Always approval should require approval for shell"
    );

    // ── (OnRequest, WorkspaceWrite) ─────────────────────────────────────────
    let on_request = PermissionEngine::new(ApprovalPolicy::OnRequest, ws_default());
    assert!(
        matches!(
            on_request.decide(&ToolRisk::read("fs.read")),
            PermissionOutcome::Allowed
        ),
        "OnRequest should allow reads"
    );
    assert!(
        matches!(
            on_request.decide(&ToolRisk::write("fs.write")),
            PermissionOutcome::Allowed
        ),
        "OnRequest should allow writes"
    );
    assert!(
        matches!(
            on_request.decide(&ToolRisk::shell("shell.exec", false)),
            PermissionOutcome::Allowed
        ),
        "OnRequest should allow non-destructive shell"
    );
    assert!(
        matches!(
            on_request.decide(&ToolRisk::destructive("rm.rf")),
            PermissionOutcome::RequiresApproval
        ),
        "OnRequest should require approval for destructive ops"
    );

    // ── (Never, DangerFullAccess); everything allowed including
    //    destructive shell. To gate destructive ops, pick OnRequest approval
    //    instead.
    let danger = PermissionEngine::new(ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess);
    assert!(matches!(
        danger.decide(&ToolRisk::read("fs.read")),
        PermissionOutcome::Allowed
    ));
    assert!(matches!(
        danger.decide(&ToolRisk::write("fs.write")),
        PermissionOutcome::Allowed
    ));
    assert!(matches!(
        danger.decide(&ToolRisk::shell("shell.exec", false)),
        PermissionOutcome::Allowed
    ));
    assert!(
        matches!(
            danger.decide(&ToolRisk::shell("shell.exec", true)),
            PermissionOutcome::Allowed
        ),
        "Never + DangerFullAccess allows destructive shell"
    );
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 3 — ShellExecTool executes a trivial command
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn shell_tool_executes_trivial_command() {
    let dir = tempfile::tempdir().unwrap();
    let tool = ShellExecTool::new(dir.path().to_path_buf());

    let inv = invocation("shell.exec", serde_json::json!({"command": "echo hello"}));
    let output = tool.invoke(inv).await.unwrap();
    assert!(
        output.text.contains("hello"),
        "output should contain 'hello', got: '{}'",
        output.text
    );
    assert!(!output.truncated);
}

// ═══════════════════════════════════════════════════════════════════════════════
// Test 4 — Full filesystem round-trip: write, read, list, + path traversal rejection
// ═══════════════════════════════════════════════════════════════════════════════

#[tokio::test]
async fn fs_read_write_list_roundtrip() {
    let dir = tempfile::tempdir().unwrap();
    let root = PathBuf::from(dir.path());

    // ── 4a. Write a file ────────────────────────────────────────────────────
    let write_tool = FsWriteTool::new(root.clone());
    let write_inv = invocation(
        "fs.write",
        serde_json::json!({
            "path": "roundtrip.txt",
            "content": "Hello, roundtrip!"
        }),
    );
    let output = write_tool.invoke(write_inv).await.unwrap();
    assert!(
        output.text.contains("roundtrip.txt"),
        "write output should mention the file: '{}'",
        output.text
    );
    assert!(!output.truncated);

    // ── 4b. Read it back ────────────────────────────────────────────────────
    let read_tool = FsReadTool::new(root.clone());
    let read_inv = invocation("fs.read", serde_json::json!({"path": "roundtrip.txt"}));
    let output = read_tool.invoke(read_inv).await.unwrap();
    assert_eq!(output.text, "Hello, roundtrip!");
    assert!(!output.truncated);

    // ── 4c. List the directory ──────────────────────────────────────────────
    let list_tool = FsListTool::new(root.clone());
    let list_inv = invocation("fs.list", serde_json::json!({"path": "."}));
    let output = list_tool.invoke(list_inv).await.unwrap();
    let entries: Vec<agent_tools::FsListEntry> = serde_json::from_str(&output.text).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "roundtrip.txt");
    assert_eq!(entries[0].entry_type, "file");

    // ── 4d. Path traversal rejection (read) ─────────────────────────────────
    // Create a workspace subdirectory + a file outside it so that
    // canonicalize() succeeds and the containment check triggers
    // WorkspaceEscape (rather than ENOENT).
    let ws_sub = dir.path().join("ws");
    std::fs::create_dir(&ws_sub).unwrap();
    let outside_file = dir.path().join("secret.txt");
    std::fs::write(&outside_file, "outside").unwrap();
    let sandboxed_read = FsReadTool::new(ws_sub);
    let traverse_inv = invocation("fs.read", serde_json::json!({"path": "../secret.txt"}));
    let result = sandboxed_read.invoke(traverse_inv).await;
    assert!(result.is_err(), "read path traversal should fail");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("escape") || err_msg.contains("WorkspaceEscape"),
        "read error should mention workspace escape, got: '{err_msg}'"
    );

    // ── 4e. Path traversal rejection (write) ────────────────────────────────
    // The write path checker catches ".." before canonicalize, so any ".."
    // path is immediately rejected as WorkspaceEscape.
    let traverse_write = invocation(
        "fs.write",
        serde_json::json!({
            "path": "../escape.txt",
            "content": "bad"
        }),
    );
    let result = write_tool.invoke(traverse_write).await;
    assert!(result.is_err(), "write path traversal should fail");
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("escape") || err_msg.contains("WorkspaceEscape"),
        "write error should mention workspace escape, got: '{err_msg}'"
    );
}
