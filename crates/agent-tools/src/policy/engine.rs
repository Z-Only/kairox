use std::collections::HashSet;
use std::path::PathBuf;

use super::approval::ApprovalPolicy;
use super::decision::{ApprovalReason, PolicyDecision};
use super::effect::{PolicyEffect, PolicyRisk};
use super::sandbox::SandboxPolicy;

enum SandboxVerdict {
    Ok,
    Reject(String),
    NeedsUpgrade(ApprovalReason),
}

#[derive(Debug, Clone)]
pub struct PolicyEngine {
    approval: ApprovalPolicy,
    sandbox: SandboxPolicy,
    workspace_root: PathBuf,
    trusted_mcp_servers: HashSet<String>,
}

impl PolicyEngine {
    pub fn new(approval: ApprovalPolicy, sandbox: SandboxPolicy, workspace_root: PathBuf) -> Self {
        Self {
            approval,
            sandbox,
            workspace_root,
            trusted_mcp_servers: HashSet::new(),
        }
    }

    pub fn approval(&self) -> ApprovalPolicy {
        self.approval
    }

    pub fn sandbox(&self) -> &SandboxPolicy {
        &self.sandbox
    }

    pub fn workspace_root(&self) -> &std::path::Path {
        &self.workspace_root
    }

    pub fn set_approval(&mut self, p: ApprovalPolicy) {
        self.approval = p;
    }

    pub fn set_sandbox(&mut self, p: SandboxPolicy) {
        self.sandbox = p;
    }

    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.workspace_root = root;
    }

    pub fn trust_mcp(&mut self, server: impl Into<String>) {
        self.trusted_mcp_servers.insert(server.into());
    }

    pub fn untrust_mcp(&mut self, server: &str) {
        self.trusted_mcp_servers.remove(server);
    }

    pub fn trusted_servers(&self) -> &HashSet<String> {
        &self.trusted_mcp_servers
    }

    pub fn decide(&self, risk: &PolicyRisk) -> PolicyDecision {
        if let PolicyEffect::McpInvoke { server } = &risk.effect {
            return self.decide_mcp(server);
        }

        let sandbox = self.sandbox_check(risk);

        match (self.approval, sandbox) {
            // Never: sandbox-only.
            (ApprovalPolicy::Never, SandboxVerdict::Ok) => PolicyDecision::Allowed,
            (ApprovalPolicy::Never, SandboxVerdict::Reject(reason)) => {
                PolicyDecision::DeniedBySandbox { reason }
            }
            (ApprovalPolicy::Never, SandboxVerdict::NeedsUpgrade(_)) => {
                PolicyDecision::DeniedBySandbox {
                    reason: "approval policy is `never` and sandbox demands escalation".into(),
                }
            }

            // OnRequest: sandbox passes → allow; sandbox wants upgrade → ask;
            // sandbox rejects → deny.
            (ApprovalPolicy::OnRequest, SandboxVerdict::Ok) => {
                if needs_destructive_review(risk) {
                    PolicyDecision::NeedsApproval {
                        reason: ApprovalReason::DestructiveEffect,
                    }
                } else {
                    PolicyDecision::Allowed
                }
            }
            (ApprovalPolicy::OnRequest, SandboxVerdict::NeedsUpgrade(reason)) => {
                PolicyDecision::NeedsApproval { reason }
            }
            (ApprovalPolicy::OnRequest, SandboxVerdict::Reject(reason)) => {
                PolicyDecision::DeniedBySandbox { reason }
            }

            // Always: prompt for anything that isn't pure read.
            (ApprovalPolicy::Always, SandboxVerdict::Ok) => {
                if matches!(risk.effect, PolicyEffect::Read) {
                    PolicyDecision::Allowed
                } else {
                    PolicyDecision::NeedsApproval {
                        reason: ApprovalReason::PolicyAlways,
                    }
                }
            }
            (ApprovalPolicy::Always, SandboxVerdict::NeedsUpgrade(reason)) => {
                PolicyDecision::NeedsApproval { reason }
            }
            (ApprovalPolicy::Always, SandboxVerdict::Reject(reason)) => {
                PolicyDecision::DeniedBySandbox { reason }
            }
        }
    }

    fn decide_mcp(&self, server: &str) -> PolicyDecision {
        if self.trusted_mcp_servers.contains(server) {
            return PolicyDecision::Allowed;
        }
        match self.approval {
            ApprovalPolicy::Never => PolicyDecision::DeniedBySandbox {
                reason: format!(
                    "MCP server `{server}` is not trusted and approval policy is `never`"
                ),
            },
            ApprovalPolicy::OnRequest | ApprovalPolicy::Always => PolicyDecision::NeedsApproval {
                reason: ApprovalReason::UntrustedMcpServer,
            },
        }
    }

    fn sandbox_check(&self, risk: &PolicyRisk) -> SandboxVerdict {
        match &risk.effect {
            PolicyEffect::Read => SandboxVerdict::Ok,
            PolicyEffect::Write { paths } => match &self.sandbox {
                SandboxPolicy::ReadOnly => {
                    SandboxVerdict::Reject("read-only sandbox blocks writes".into())
                }
                SandboxPolicy::DangerFullAccess => SandboxVerdict::Ok,
                SandboxPolicy::WorkspaceWrite { .. } => {
                    if paths.is_empty() {
                        SandboxVerdict::Ok
                    } else {
                        let bad: Vec<String> = paths
                            .iter()
                            .filter(|p| !self.sandbox.path_writable(p, &self.workspace_root))
                            .map(|p| p.display().to_string())
                            .collect();
                        if bad.is_empty() {
                            SandboxVerdict::Ok
                        } else {
                            SandboxVerdict::NeedsUpgrade(ApprovalReason::SandboxRejected)
                        }
                    }
                }
            },
            PolicyEffect::Shell { destructive } => match &self.sandbox {
                SandboxPolicy::ReadOnly => {
                    SandboxVerdict::Reject("read-only sandbox blocks shell execution".into())
                }
                SandboxPolicy::DangerFullAccess => SandboxVerdict::Ok,
                SandboxPolicy::WorkspaceWrite { .. } => {
                    if *destructive {
                        SandboxVerdict::NeedsUpgrade(ApprovalReason::DestructiveEffect)
                    } else {
                        SandboxVerdict::Ok
                    }
                }
            },
            PolicyEffect::Network { .. } => match &self.sandbox {
                SandboxPolicy::ReadOnly => {
                    SandboxVerdict::Reject("read-only sandbox blocks network access".into())
                }
                SandboxPolicy::DangerFullAccess => SandboxVerdict::Ok,
                SandboxPolicy::WorkspaceWrite { network_access, .. } => {
                    if *network_access {
                        SandboxVerdict::Ok
                    } else {
                        SandboxVerdict::NeedsUpgrade(ApprovalReason::NetworkRequest)
                    }
                }
            },
            PolicyEffect::Destructive => match &self.sandbox {
                SandboxPolicy::ReadOnly => {
                    SandboxVerdict::Reject("read-only sandbox blocks destructive operations".into())
                }
                SandboxPolicy::DangerFullAccess => SandboxVerdict::Ok,
                SandboxPolicy::WorkspaceWrite { .. } => {
                    SandboxVerdict::NeedsUpgrade(ApprovalReason::DestructiveEffect)
                }
            },
            PolicyEffect::McpInvoke { .. } => unreachable!("handled before sandbox_check"),
        }
    }
}

fn needs_destructive_review(risk: &PolicyRisk) -> bool {
    matches!(
        risk.effect,
        PolicyEffect::Destructive | PolicyEffect::Shell { destructive: true }
    )
}

#[cfg(test)]
mod tests {
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
}
