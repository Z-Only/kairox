use super::*;

fn ws_default() -> SandboxPolicy {
    SandboxPolicy::WorkspaceWrite {
        network_access: false,
        writable_roots: vec![],
    }
}

#[test]
fn readonly_allows_reads_and_blocks_shell_writes() {
    let engine = PermissionEngine::new(ApprovalPolicy::Never, SandboxPolicy::ReadOnly);
    assert_eq!(
        engine.decide(&ToolRisk::read("fs.read")),
        PermissionOutcome::Allowed
    );
    assert!(matches!(
        engine.decide(&ToolRisk::write("fs.write")),
        PermissionOutcome::Denied(_)
    ));
    assert!(matches!(
        engine.decide(&ToolRisk::shell("shell.exec", false)),
        PermissionOutcome::Denied(_)
    ));
}

#[test]
fn always_requires_approval_for_effectful_tools() {
    let engine = PermissionEngine::new(ApprovalPolicy::Always, ws_default());
    assert_eq!(
        engine.decide(&ToolRisk::write("patch.apply")),
        PermissionOutcome::RequiresApproval
    );
}

#[test]
fn never_plus_danger_allows_destructive() {
    let engine = PermissionEngine::new(ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess);
    assert_eq!(
        engine.decide(&ToolRisk::shell("shell.exec", true)),
        PermissionOutcome::Allowed
    );
    assert_eq!(
        engine.decide(&ToolRisk::destructive("rm.rf")),
        PermissionOutcome::Allowed
    );
}

#[test]
fn destructive_denied_under_readonly_sandbox() {
    let engine = PermissionEngine::new(ApprovalPolicy::Never, SandboxPolicy::ReadOnly);
    assert!(matches!(
        engine.decide(&ToolRisk::destructive("rm.rf")),
        PermissionOutcome::Denied(_)
    ));
}

#[test]
fn on_request_pends_destructive_in_workspace_write() {
    let engine = PermissionEngine::new(ApprovalPolicy::OnRequest, ws_default());
    assert_eq!(
        engine.decide(&ToolRisk::destructive("rm.rf")),
        PermissionOutcome::RequiresApproval
    );
}

#[test]
fn mcp_untrusted_server_under_never_denies() {
    let engine = PermissionEngine::new(ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess);
    let outcome = engine.check_mcp_permission("unknown-server", "some-tool");
    assert!(matches!(outcome, PermissionOutcome::Denied(_)));
}

#[test]
fn mcp_untrusted_server_under_on_request_prompts_for_trust() {
    let engine = PermissionEngine::new(ApprovalPolicy::OnRequest, ws_default());
    assert_eq!(
        engine.check_mcp_permission("unknown-server", "some-tool"),
        PermissionOutcome::PromptWithTrust
    );
}

#[test]
fn mcp_trusted_server_always_allowed() {
    let mut engine = PermissionEngine::new(ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess);
    engine.trust_server("my-server".into());
    assert_eq!(
        engine.check_mcp_permission("my-server", "tool"),
        PermissionOutcome::Allowed
    );

    let mut engine = PermissionEngine::new(ApprovalPolicy::Never, SandboxPolicy::ReadOnly);
    engine.trust_server("my-server".into());
    assert_eq!(
        engine.check_mcp_permission("my-server", "tool"),
        PermissionOutcome::Allowed
    );
}

#[test]
fn trust_and_revoke_roundtrip() {
    let mut engine = PermissionEngine::new(ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess);
    engine.trust_server("srv-a".into());
    engine.trust_server("srv-b".into());
    assert!(engine.trusted_servers().contains("srv-a"));
    assert!(engine.trusted_servers().contains("srv-b"));

    engine.revoke_trust("srv-a");
    assert!(!engine.trusted_servers().contains("srv-a"));
    assert!(engine.trusted_servers().contains("srv-b"));
}

#[test]
fn set_approval_keeps_sandbox() {
    let mut engine = PermissionEngine::new(ApprovalPolicy::OnRequest, ws_default());
    let before = engine.sandbox_policy().clone();
    engine.set_approval_policy(ApprovalPolicy::Always);
    assert_eq!(engine.approval_policy(), ApprovalPolicy::Always);
    assert_eq!(engine.sandbox_policy(), &before);
}

#[test]
fn set_sandbox_keeps_approval() {
    let mut engine = PermissionEngine::new(ApprovalPolicy::OnRequest, ws_default());
    let before = engine.approval_policy();
    engine.set_sandbox_policy(SandboxPolicy::DangerFullAccess);
    assert_eq!(engine.approval_policy(), before);
    assert_eq!(engine.sandbox_policy(), &SandboxPolicy::DangerFullAccess);
}
