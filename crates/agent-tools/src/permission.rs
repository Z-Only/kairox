//! Permission outcomes, tool risks, and the [`PermissionEngine`] thin wrapper
//! around [`crate::policy::PolicyEngine`].
//!
//! Construct [`PermissionEngine`] from an `(ApprovalPolicy, SandboxPolicy)`
//! pair.

use std::collections::HashSet;
use std::path::PathBuf;

use crate::policy::{
    ApprovalPolicy, ApprovalReason, PolicyDecision, PolicyEffect, PolicyEngine, PolicyRisk,
    SandboxPolicy,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionOutcome {
    Allowed,
    RequiresApproval,
    Pending,
    Denied(String),
    PromptWithTrust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolEffect {
    Read,
    Write,
    Shell { destructive: bool },
    Network,
    Destructive,
    McpInvoke,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRisk {
    pub tool_id: String,
    pub effect: ToolEffect,
}

impl ToolRisk {
    pub fn read(tool_id: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: ToolEffect::Read,
        }
    }

    pub fn write(tool_id: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: ToolEffect::Write,
        }
    }

    pub fn shell(tool_id: impl Into<String>, destructive: bool) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: ToolEffect::Shell { destructive },
        }
    }

    pub fn destructive(tool_id: impl Into<String>) -> Self {
        Self {
            tool_id: tool_id.into(),
            effect: ToolEffect::Destructive,
        }
    }
}

fn to_policy_risk(risk: &ToolRisk, mcp_server_hint: Option<&str>) -> PolicyRisk {
    let effect = match (risk.effect, mcp_server_hint) {
        (ToolEffect::Read, _) => PolicyEffect::Read,
        (ToolEffect::Write, _) => PolicyEffect::Write { paths: Vec::new() },
        (ToolEffect::Shell { destructive }, _) => PolicyEffect::Shell { destructive },
        (ToolEffect::Network, _) => PolicyEffect::Network { hosts: Vec::new() },
        (ToolEffect::Destructive, _) => PolicyEffect::Destructive,
        (ToolEffect::McpInvoke, Some(server)) => PolicyEffect::McpInvoke {
            server: server.to_string(),
        },
        (ToolEffect::McpInvoke, None) => PolicyEffect::McpInvoke {
            server: String::new(),
        },
    };
    PolicyRisk {
        tool_id: risk.tool_id.clone(),
        effect,
    }
}

fn from_policy_decision(decision: PolicyDecision) -> PermissionOutcome {
    match decision {
        PolicyDecision::Allowed => PermissionOutcome::Allowed,
        PolicyDecision::DeniedBySandbox { reason } => PermissionOutcome::Denied(reason),
        PolicyDecision::NeedsApproval { reason } => match reason {
            ApprovalReason::UntrustedMcpServer => PermissionOutcome::PromptWithTrust,
            _ => PermissionOutcome::RequiresApproval,
        },
    }
}

#[derive(Debug, Clone)]
pub struct PermissionEngine {
    policy: PolicyEngine,
}

impl Default for PermissionEngine {
    fn default() -> Self {
        Self::new(ApprovalPolicy::default(), SandboxPolicy::default())
    }
}

impl PermissionEngine {
    pub fn new(approval: ApprovalPolicy, sandbox: SandboxPolicy) -> Self {
        Self {
            policy: PolicyEngine::new(approval, sandbox, PathBuf::new()),
        }
    }

    pub fn approval_policy(&self) -> ApprovalPolicy {
        self.policy.approval()
    }

    pub fn sandbox_policy(&self) -> &SandboxPolicy {
        self.policy.sandbox()
    }

    pub fn set_approval_policy(&mut self, approval: ApprovalPolicy) {
        self.policy.set_approval(approval);
    }

    pub fn set_sandbox_policy(&mut self, sandbox: SandboxPolicy) {
        self.policy.set_sandbox(sandbox);
    }

    pub fn set_workspace_root(&mut self, root: PathBuf) {
        self.policy.set_workspace_root(root);
    }

    pub fn policy_engine(&self) -> &PolicyEngine {
        &self.policy
    }

    pub fn check_mcp_permission(&self, server_id: &str, tool_id: &str) -> PermissionOutcome {
        let risk = ToolRisk {
            tool_id: tool_id.to_string(),
            effect: ToolEffect::McpInvoke,
        };
        let policy_risk = to_policy_risk(&risk, Some(server_id));
        from_policy_decision(self.policy.decide(&policy_risk))
    }

    pub fn trust_server(&mut self, server_id: String) {
        self.policy.trust_mcp(server_id);
    }

    pub fn revoke_trust(&mut self, server_id: &str) {
        self.policy.untrust_mcp(server_id);
    }

    pub fn trusted_servers(&self) -> &HashSet<String> {
        self.policy.trusted_servers()
    }

    pub fn decide(&self, risk: &ToolRisk) -> PermissionOutcome {
        let policy_risk = to_policy_risk(risk, None);
        from_policy_decision(self.policy.decide(&policy_risk))
    }
}

#[cfg(test)]
mod tests {
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
        let mut engine =
            PermissionEngine::new(ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess);
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
        let mut engine =
            PermissionEngine::new(ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess);
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
}
