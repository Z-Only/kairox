use super::*;

fn engine(approval: ApprovalPolicy, sandbox: SandboxPolicy) -> PolicyEngine {
    PolicyEngine::new(approval, sandbox, PathBuf::from("/ws"))
}

#[test]
fn read_always_allowed_under_any_combo() {
    for a in [
        ApprovalPolicy::Never,
        ApprovalPolicy::OnRequest,
        ApprovalPolicy::Always,
    ] {
        for s in [
            SandboxPolicy::ReadOnly,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
            SandboxPolicy::DangerFullAccess,
        ] {
            let e = engine(a, s.clone());
            assert_eq!(
                e.decide(&PolicyRisk::read("fs.read")),
                PolicyDecision::Allowed,
                "approval={a:?} sandbox={s:?}"
            );
        }
    }
}

#[test]
fn readonly_sandbox_rejects_writes() {
    let e = engine(ApprovalPolicy::Always, SandboxPolicy::ReadOnly);
    match e.decide(&PolicyRisk::write("fs.write")) {
        PolicyDecision::DeniedBySandbox { .. } => {}
        other => panic!("expected DeniedBySandbox, got {other:?}"),
    }
}

#[test]
fn never_plus_workspace_write_allows_non_destructive_writes() {
    let e = engine(
        ApprovalPolicy::Never,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        },
    );
    assert_eq!(
        e.decide(&PolicyRisk::write("fs.write")),
        PolicyDecision::Allowed
    );
    assert_eq!(
        e.decide(&PolicyRisk::shell("shell.exec", false)),
        PolicyDecision::Allowed
    );
}

#[test]
fn never_plus_workspace_write_denies_destructive_shell() {
    let e = engine(
        ApprovalPolicy::Never,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        },
    );
    match e.decide(&PolicyRisk::shell("shell.exec", true)) {
        PolicyDecision::DeniedBySandbox { .. } => {}
        other => panic!("expected DeniedBySandbox, got {other:?}"),
    }
}

#[test]
fn on_request_upgrades_destructive() {
    let e = engine(
        ApprovalPolicy::OnRequest,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        },
    );
    assert_eq!(
        e.decide(&PolicyRisk::destructive("rm")),
        PolicyDecision::NeedsApproval {
            reason: ApprovalReason::DestructiveEffect
        }
    );
}

#[test]
fn on_request_upgrades_network_when_disabled() {
    let e = engine(
        ApprovalPolicy::OnRequest,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        },
    );
    assert_eq!(
        e.decide(&PolicyRisk::network(
            "http.fetch",
            vec!["example.com".into()]
        )),
        PolicyDecision::NeedsApproval {
            reason: ApprovalReason::NetworkRequest
        }
    );
}

#[test]
fn always_prompts_for_non_read() {
    let e = engine(
        ApprovalPolicy::Always,
        SandboxPolicy::WorkspaceWrite {
            network_access: true,
            writable_roots: vec![],
        },
    );
    assert_eq!(
        e.decide(&PolicyRisk::write("fs.write")),
        PolicyDecision::NeedsApproval {
            reason: ApprovalReason::PolicyAlways
        }
    );
}

#[test]
fn danger_full_access_allows_destructive_when_never() {
    let e = engine(ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess);
    assert_eq!(
        e.decide(&PolicyRisk::destructive("rm")),
        PolicyDecision::Allowed
    );
}

#[test]
fn danger_full_access_still_prompts_destructive_under_on_request() {
    let e = engine(ApprovalPolicy::OnRequest, SandboxPolicy::DangerFullAccess);
    assert_eq!(
        e.decide(&PolicyRisk::destructive("rm")),
        PolicyDecision::NeedsApproval {
            reason: ApprovalReason::DestructiveEffect
        }
    );
}

#[test]
fn workspace_write_path_outside_root_needs_upgrade() {
    let e = engine(
        ApprovalPolicy::OnRequest,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        },
    );
    let risk = PolicyRisk::write_paths("fs.write", vec![PathBuf::from("/elsewhere/x.txt")]);
    assert_eq!(
        e.decide(&risk),
        PolicyDecision::NeedsApproval {
            reason: ApprovalReason::SandboxRejected
        }
    );
}

#[test]
fn workspace_write_path_in_extra_root_passes() {
    let mut e = engine(
        ApprovalPolicy::Never,
        SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![PathBuf::from("/tmp/cache")],
        },
    );
    e.set_workspace_root(PathBuf::from("/ws"));
    let risk = PolicyRisk::write_paths("fs.write", vec![PathBuf::from("/tmp/cache/x.txt")]);
    assert_eq!(e.decide(&risk), PolicyDecision::Allowed);
}

#[test]
fn mcp_trusted_allowed() {
    let mut e = engine(ApprovalPolicy::Never, SandboxPolicy::ReadOnly);
    e.trust_mcp("github");
    assert_eq!(
        e.decide(&PolicyRisk::mcp("mcp.invoke", "github")),
        PolicyDecision::Allowed
    );
}

#[test]
fn mcp_untrusted_under_never_denied() {
    let e = engine(ApprovalPolicy::Never, SandboxPolicy::ReadOnly);
    match e.decide(&PolicyRisk::mcp("mcp.invoke", "ghost")) {
        PolicyDecision::DeniedBySandbox { .. } => {}
        other => panic!("expected DeniedBySandbox, got {other:?}"),
    }
}

#[test]
fn mcp_untrusted_under_on_request_needs_approval() {
    let e = engine(ApprovalPolicy::OnRequest, SandboxPolicy::ReadOnly);
    assert_eq!(
        e.decide(&PolicyRisk::mcp("mcp.invoke", "ghost")),
        PolicyDecision::NeedsApproval {
            reason: ApprovalReason::UntrustedMcpServer
        }
    );
}

#[test]
fn untrust_removes_from_set() {
    let mut e = engine(ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess);
    e.trust_mcp("github");
    e.untrust_mcp("github");
    assert!(!e.trusted_servers().contains("github"));
}
