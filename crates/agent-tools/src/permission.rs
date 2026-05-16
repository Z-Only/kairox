use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::str::FromStr;

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

#[derive(Debug, Clone)]
pub struct PermissionEngine {
    mode: PermissionMode,
    trusted_mcp_servers: HashSet<String>,
}

impl PermissionEngine {
    pub fn new(mode: PermissionMode) -> Self {
        Self {
            mode,
            trusted_mcp_servers: HashSet::new(),
        }
    }

    pub fn mode(&self) -> &PermissionMode {
        &self.mode
    }

    pub fn set_mode(&mut self, mode: PermissionMode) {
        self.mode = mode;
    }

    pub fn check_mcp_permission(&self, server_id: &str, _tool_id: &str) -> PermissionOutcome {
        if self.trusted_mcp_servers.contains(server_id) {
            match self.mode {
                PermissionMode::ReadOnly => {
                    PermissionOutcome::Denied("read-only mode blocks MCP tools".into())
                }
                PermissionMode::Autonomous => PermissionOutcome::Allowed,
                _ => PermissionOutcome::RequiresApproval,
            }
        } else {
            PermissionOutcome::PromptWithTrust
        }
    }

    pub fn trust_server(&mut self, server_id: String) {
        self.trusted_mcp_servers.insert(server_id);
    }

    pub fn revoke_trust(&mut self, server_id: &str) {
        self.trusted_mcp_servers.remove(server_id);
    }

    pub fn trusted_servers(&self) -> &HashSet<String> {
        &self.trusted_mcp_servers
    }

    pub fn decide(&self, risk: &ToolRisk) -> PermissionOutcome {
        match (self.mode, &risk.effect) {
            (PermissionMode::ReadOnly, ToolEffect::Read) => PermissionOutcome::Allowed,
            (PermissionMode::ReadOnly, ToolEffect::Write) => {
                PermissionOutcome::Denied("read-only mode blocks writes".into())
            }
            (PermissionMode::ReadOnly, ToolEffect::Shell { .. }) => {
                PermissionOutcome::Denied("read-only mode blocks shell execution".into())
            }
            (PermissionMode::ReadOnly, ToolEffect::Network) => {
                PermissionOutcome::Denied("read-only mode blocks network access".into())
            }
            (PermissionMode::ReadOnly, ToolEffect::Destructive) => {
                PermissionOutcome::Denied("read-only mode blocks destructive operations".into())
            }
            (PermissionMode::ReadOnly, ToolEffect::McpInvoke) => {
                PermissionOutcome::Denied("read-only mode blocks MCP tools".into())
            }
            (PermissionMode::Suggest, ToolEffect::Read) => PermissionOutcome::Allowed,
            (PermissionMode::Suggest, _) => PermissionOutcome::RequiresApproval,
            (PermissionMode::Agent, ToolEffect::Read) => PermissionOutcome::Allowed,
            (PermissionMode::Agent, ToolEffect::Write) => PermissionOutcome::Allowed,
            (PermissionMode::Agent, ToolEffect::Shell { destructive: false }) => {
                PermissionOutcome::Allowed
            }
            (PermissionMode::Agent, ToolEffect::Destructive) => PermissionOutcome::RequiresApproval,
            (PermissionMode::Agent, _) => PermissionOutcome::RequiresApproval,
            (PermissionMode::Interactive, ToolEffect::Read) => PermissionOutcome::Allowed,
            (PermissionMode::Interactive, ToolEffect::Write) => PermissionOutcome::Pending,
            (PermissionMode::Interactive, ToolEffect::Shell { .. }) => PermissionOutcome::Pending,
            (PermissionMode::Interactive, ToolEffect::Network) => PermissionOutcome::Pending,
            (PermissionMode::Interactive, ToolEffect::Destructive) => PermissionOutcome::Pending,
            (PermissionMode::Interactive, ToolEffect::McpInvoke) => PermissionOutcome::Pending,
            (PermissionMode::Autonomous, ToolEffect::Shell { destructive: true }) => {
                PermissionOutcome::RequiresApproval
            }
            (PermissionMode::Autonomous, ToolEffect::Network) => {
                PermissionOutcome::RequiresApproval
            }
            (PermissionMode::Autonomous, ToolEffect::Destructive) => {
                PermissionOutcome::RequiresApproval
            }
            (PermissionMode::Autonomous, _) => PermissionOutcome::Allowed,
        }
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
        assert_eq!(
            engine.decide(&ToolRisk::write("fs.write")),
            PermissionOutcome::Denied("read-only mode blocks writes".into())
        );
        assert_eq!(
            engine.decide(&ToolRisk::shell("shell.exec", false)),
            PermissionOutcome::Denied("read-only mode blocks shell execution".into())
        );
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

        assert_eq!(
            engine.decide(&ToolRisk::shell("shell.exec", true)),
            PermissionOutcome::RequiresApproval
        );
    }

    #[test]
    fn destructive_risk_requires_approval_even_in_autonomous_mode() {
        let engine = PermissionEngine::new(PermissionMode::Autonomous);
        let risk = ToolRisk::destructive("rm.rf");
        assert_eq!(engine.decide(&risk), PermissionOutcome::RequiresApproval);
    }

    #[test]
    fn destructive_risk_denied_in_readonly_mode() {
        let engine = PermissionEngine::new(PermissionMode::ReadOnly);
        let risk = ToolRisk::destructive("rm.rf");
        assert_eq!(
            engine.decide(&risk),
            PermissionOutcome::Denied("read-only mode blocks destructive operations".into())
        );
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
        assert_eq!(
            engine.decide(&ToolRisk::write("fs.write")),
            PermissionOutcome::Pending
        );
        assert_eq!(
            engine.decide(&ToolRisk::shell("shell.exec", false)),
            PermissionOutcome::Pending
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
        let outcome = engine.check_mcp_permission("unknown-server", "some-tool");
        assert_eq!(outcome, PermissionOutcome::PromptWithTrust);
    }

    #[test]
    fn mcp_trusted_server_autonomous_allows() {
        let mut engine = PermissionEngine::new(PermissionMode::Autonomous);
        engine.trust_server("my-server".into());
        let outcome = engine.check_mcp_permission("my-server", "some-tool");
        assert_eq!(outcome, PermissionOutcome::Allowed);
    }

    #[test]
    fn mcp_trusted_server_readonly_denies() {
        let mut engine = PermissionEngine::new(PermissionMode::ReadOnly);
        engine.trust_server("my-server".into());
        let outcome = engine.check_mcp_permission("my-server", "some-tool");
        assert_eq!(
            outcome,
            PermissionOutcome::Denied("read-only mode blocks MCP tools".into())
        );
    }

    #[test]
    fn mcp_trusted_server_suggest_requires_approval() {
        let mut engine = PermissionEngine::new(PermissionMode::Suggest);
        engine.trust_server("my-server".into());
        let outcome = engine.check_mcp_permission("my-server", "some-tool");
        assert_eq!(outcome, PermissionOutcome::RequiresApproval);
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
}
