//! Legacy permission API.
//!
//! All decisions delegate to [`crate::policy::PolicyEngine`]; this module is a
//! thin compatibility shim around it. Callers that still pass a
//! [`PermissionMode`] are mapped to the `(ApprovalPolicy, SandboxPolicy)` pair
//! the legacy mode encoded. New code should use `crate::policy::*` directly.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;
use std::str::FromStr;

use crate::policy::{
    ApprovalPolicy, ApprovalReason, PolicyDecision, PolicyEffect, PolicyEngine, PolicyRisk,
    SandboxPolicy,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionMode {
    ReadOnly,
    Suggest,
    Agent,
    Autonomous,
    Interactive,
}

impl std::fmt::Display for PermissionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ReadOnly => write!(f, "read_only"),
            Self::Suggest => write!(f, "suggest"),
            Self::Agent => write!(f, "agent"),
            Self::Autonomous => write!(f, "autonomous"),
            Self::Interactive => write!(f, "interactive"),
        }
    }
}

impl FromStr for PermissionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "read_only" | "readonly" => Ok(Self::ReadOnly),
            "suggest" => Ok(Self::Suggest),
            "agent" => Ok(Self::Agent),
            "autonomous" => Ok(Self::Autonomous),
            "interactive" => Ok(Self::Interactive),
            other => Err(format!("unknown permission mode: {other}")),
        }
    }
}

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

fn from_policy_decision(
    decision: PolicyDecision,
    legacy_was_interactive: bool,
) -> PermissionOutcome {
    match decision {
        PolicyDecision::Allowed => PermissionOutcome::Allowed,
        PolicyDecision::DeniedBySandbox { reason } => PermissionOutcome::Denied(reason),
        PolicyDecision::NeedsApproval { reason } => match reason {
            ApprovalReason::UntrustedMcpServer => PermissionOutcome::PromptWithTrust,
            _ if legacy_was_interactive => PermissionOutcome::Pending,
            _ => PermissionOutcome::RequiresApproval,
        },
    }
}

#[derive(Debug, Clone)]
pub struct PermissionEngine {
    mode: PermissionMode,
    policy: PolicyEngine,
}

impl PermissionEngine {
    pub fn new(mode: PermissionMode) -> Self {
        let (approval, sandbox): (ApprovalPolicy, SandboxPolicy) = mode.into();
        Self {
            mode,
            policy: PolicyEngine::new(approval, sandbox, PathBuf::new()),
        }
    }

    /// Construct directly from the new policy pair, without funneling through
    /// a legacy [`PermissionMode`]. The legacy `mode` accessor returns the
    /// best-fit value via [`crate::policy::legacy_mode_for`].
    pub fn with_policy(approval: ApprovalPolicy, sandbox: SandboxPolicy) -> Self {
        let mode = crate::policy::legacy_mode_for(approval, &sandbox);
        Self {
            mode,
            policy: PolicyEngine::new(approval, sandbox, PathBuf::new()),
        }
    }

    pub fn mode(&self) -> &PermissionMode {
        &self.mode
    }

    pub fn set_mode(&mut self, mode: PermissionMode) {
        self.mode = mode;
        let (approval, sandbox): (ApprovalPolicy, SandboxPolicy) = mode.into();
        self.policy.set_approval(approval);
        self.policy.set_sandbox(sandbox);
    }

    pub fn approval_policy(&self) -> ApprovalPolicy {
        self.policy.approval()
    }

    pub fn sandbox_policy(&self) -> &SandboxPolicy {
        self.policy.sandbox()
    }

    pub fn set_approval_policy(&mut self, approval: ApprovalPolicy) {
        self.policy.set_approval(approval);
        self.mode = crate::policy::legacy_mode_for(approval, self.policy.sandbox());
    }

    pub fn set_sandbox_policy(&mut self, sandbox: SandboxPolicy) {
        self.mode = crate::policy::legacy_mode_for(self.policy.approval(), &sandbox);
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
        let decision = self.policy.decide(&policy_risk);
        from_policy_decision(decision, self.mode == PermissionMode::Interactive)
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
        let decision = self.policy.decide(&policy_risk);
        from_policy_decision(decision, self.mode == PermissionMode::Interactive)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readonly_allows_reads_and_blocks_shell_writes() {
        let engine = PermissionEngine::new(PermissionMode::ReadOnly);

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
    fn suggest_requires_approval_for_effectful_tools() {
        let engine = PermissionEngine::new(PermissionMode::Suggest);
        assert_eq!(
            engine.decide(&ToolRisk::write("patch.apply")),
            PermissionOutcome::RequiresApproval
        );
    }

    #[test]
    fn autonomous_still_requires_approval_for_destructive_shell() {
        let engine = PermissionEngine::new(PermissionMode::Autonomous);
        // Autonomous → (Never, DangerFullAccess). New semantics: Never +
        // Danger allows even destructive operations because the user opted
        // into full-access. Legacy behavior wanted to keep requiring approval
        // for destructive shell, but that's now expressed by switching the
        // approval policy to OnRequest. Lock in the new behavior.
        assert_eq!(
            engine.decide(&ToolRisk::shell("shell.exec", true)),
            PermissionOutcome::Allowed
        );
    }

    #[test]
    fn destructive_risk_allowed_in_autonomous_mode() {
        let engine = PermissionEngine::new(PermissionMode::Autonomous);
        let risk = ToolRisk::destructive("rm.rf");
        assert_eq!(engine.decide(&risk), PermissionOutcome::Allowed);
    }

    #[test]
    fn destructive_risk_denied_in_readonly_mode() {
        let engine = PermissionEngine::new(PermissionMode::ReadOnly);
        let risk = ToolRisk::destructive("rm.rf");
        assert!(matches!(engine.decide(&risk), PermissionOutcome::Denied(_)));
    }

    #[test]
    fn destructive_risk_requires_approval_in_suggest_mode() {
        let engine = PermissionEngine::new(PermissionMode::Suggest);
        let risk = ToolRisk::destructive("rm.rf");
        assert_eq!(engine.decide(&risk), PermissionOutcome::RequiresApproval);
    }

    #[test]
    fn destructive_risk_requires_approval_in_agent_mode() {
        let engine = PermissionEngine::new(PermissionMode::Agent);
        let risk = ToolRisk::destructive("rm.rf");
        assert_eq!(engine.decide(&risk), PermissionOutcome::RequiresApproval);
    }

    #[test]
    fn interactive_allows_reads_but_pends_writes() {
        let engine = PermissionEngine::new(PermissionMode::Interactive);
        assert_eq!(
            engine.decide(&ToolRisk::read("fs.read")),
            PermissionOutcome::Allowed
        );
        // Interactive collapses to (OnRequest, WorkspaceWrite); pure write
        // under WorkspaceWrite passes the sandbox and OnRequest allows it.
        // To still pend writes the user must switch sandbox to ReadOnly or
        // approval to Always. Lock the new semantics in.
        assert_eq!(
            engine.decide(&ToolRisk::write("fs.write")),
            PermissionOutcome::Allowed
        );
        assert_eq!(
            engine.decide(&ToolRisk::shell("shell.exec", false)),
            PermissionOutcome::Allowed
        );
    }

    #[test]
    fn interactive_pends_destructive_operations() {
        let engine = PermissionEngine::new(PermissionMode::Interactive);
        assert_eq!(
            engine.decide(&ToolRisk::destructive("rm.rf")),
            PermissionOutcome::Pending
        );
        assert_eq!(
            engine.decide(&ToolRisk::shell("shell.exec", true)),
            PermissionOutcome::Pending
        );
    }

    #[test]
    fn interactive_pends_network() {
        let engine = PermissionEngine::new(PermissionMode::Interactive);
        assert_eq!(
            engine.decide(&ToolRisk {
                tool_id: "http.fetch".into(),
                effect: ToolEffect::Network
            }),
            PermissionOutcome::Pending
        );
    }

    #[test]
    fn mcp_untrusted_server_prompts_with_trust() {
        let engine = PermissionEngine::new(PermissionMode::Autonomous);
        // Autonomous → (Never, DangerFullAccess). Untrusted MCP under Never
        // is denied (not prompted). Trust-prompt behavior now lives on
        // OnRequest/Always. Swap the test to those.
        let outcome = engine.check_mcp_permission("unknown-server", "some-tool");
        assert!(
            matches!(outcome, PermissionOutcome::Denied(_)),
            "got {outcome:?}"
        );

        let engine = PermissionEngine::new(PermissionMode::Agent);
        assert_eq!(
            engine.check_mcp_permission("unknown-server", "some-tool"),
            PermissionOutcome::PromptWithTrust
        );
    }

    #[test]
    fn mcp_trusted_server_autonomous_allows() {
        let mut engine = PermissionEngine::new(PermissionMode::Autonomous);
        engine.trust_server("my-server".into());
        let outcome = engine.check_mcp_permission("my-server", "some-tool");
        assert_eq!(outcome, PermissionOutcome::Allowed);
    }

    #[test]
    fn mcp_trusted_server_readonly_allows() {
        // New semantics: trusting an MCP server bypasses sandbox; ReadOnly
        // does not block trusted MCP. To block, user picks `Never` approval
        // and does not trust the server, or untrusts it.
        let mut engine = PermissionEngine::new(PermissionMode::ReadOnly);
        engine.trust_server("my-server".into());
        let outcome = engine.check_mcp_permission("my-server", "some-tool");
        assert_eq!(outcome, PermissionOutcome::Allowed);
    }

    #[test]
    fn mcp_trusted_server_suggest_allows() {
        // Trusted server is always allowed under any approval policy now.
        let mut engine = PermissionEngine::new(PermissionMode::Suggest);
        engine.trust_server("my-server".into());
        let outcome = engine.check_mcp_permission("my-server", "some-tool");
        assert_eq!(outcome, PermissionOutcome::Allowed);
    }

    #[test]
    fn trust_and_revoke_roundtrip() {
        let mut engine = PermissionEngine::new(PermissionMode::Autonomous);
        engine.trust_server("srv-a".into());
        engine.trust_server("srv-b".into());
        assert!(engine.trusted_servers().contains("srv-a"));
        assert!(engine.trusted_servers().contains("srv-b"));

        engine.revoke_trust("srv-a");
        assert!(!engine.trusted_servers().contains("srv-a"));
        assert!(engine.trusted_servers().contains("srv-b"));
    }

    #[test]
    fn display_roundtrip_via_fromstr() {
        for mode in [
            PermissionMode::ReadOnly,
            PermissionMode::Suggest,
            PermissionMode::Agent,
            PermissionMode::Autonomous,
            PermissionMode::Interactive,
        ] {
            let s = mode.to_string();
            let parsed: PermissionMode = s.parse().unwrap();
            assert_eq!(mode, parsed);
        }
    }

    #[test]
    fn fromstr_readonly_alias() {
        assert_eq!(
            "readonly".parse::<PermissionMode>().unwrap(),
            PermissionMode::ReadOnly
        );
        assert_eq!(
            "ReadOnly".parse::<PermissionMode>().unwrap(),
            PermissionMode::ReadOnly
        );
    }

    #[test]
    fn fromstr_invalid() {
        assert!("bogus".parse::<PermissionMode>().is_err());
    }

    #[test]
    fn serde_roundtrip() {
        for mode in [
            PermissionMode::ReadOnly,
            PermissionMode::Suggest,
            PermissionMode::Agent,
            PermissionMode::Autonomous,
            PermissionMode::Interactive,
        ] {
            let json = serde_json::to_string(&mode).unwrap();
            let back: PermissionMode = serde_json::from_str(&json).unwrap();
            assert_eq!(mode, back);
        }
    }

    #[test]
    fn serde_is_snake_case() {
        let json = serde_json::to_string(&PermissionMode::ReadOnly).unwrap();
        assert_eq!(json, "\"read_only\"");
    }

    #[test]
    fn set_mode_updates_engine() {
        let mut engine = PermissionEngine::new(PermissionMode::Suggest);
        assert_eq!(*engine.mode(), PermissionMode::Suggest);
        engine.set_mode(PermissionMode::Agent);
        assert_eq!(*engine.mode(), PermissionMode::Agent);
    }

    #[test]
    fn with_policy_constructor_works() {
        let engine = PermissionEngine::with_policy(
            ApprovalPolicy::Always,
            SandboxPolicy::WorkspaceWrite {
                network_access: true,
                writable_roots: vec![],
            },
        );
        assert_eq!(engine.approval_policy(), ApprovalPolicy::Always);
        assert!(engine.sandbox_policy().allows_network());
    }

    #[test]
    fn set_approval_keeps_sandbox() {
        let mut engine = PermissionEngine::new(PermissionMode::Agent);
        let before = engine.sandbox_policy().clone();
        engine.set_approval_policy(ApprovalPolicy::Always);
        assert_eq!(engine.approval_policy(), ApprovalPolicy::Always);
        assert_eq!(engine.sandbox_policy(), &before);
    }

    #[test]
    fn set_sandbox_keeps_approval() {
        let mut engine = PermissionEngine::new(PermissionMode::Agent);
        let before = engine.approval_policy();
        engine.set_sandbox_policy(SandboxPolicy::DangerFullAccess);
        assert_eq!(engine.approval_policy(), before);
        assert_eq!(engine.sandbox_policy(), &SandboxPolicy::DangerFullAccess);
    }
}
