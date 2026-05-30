use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalReason {
    SandboxRejected,
    PolicyAlways,
    DestructiveEffect,
    UnknownCommand,
    NetworkRequest,
    UntrustedMcpServer,
}

impl ApprovalReason {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SandboxRejected => "sandbox_rejected",
            Self::PolicyAlways => "policy_always",
            Self::DestructiveEffect => "destructive_effect",
            Self::UnknownCommand => "unknown_command",
            Self::NetworkRequest => "network_request",
            Self::UntrustedMcpServer => "untrusted_mcp_server",
        }
    }
}

impl fmt::Display for ApprovalReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    Allowed,
    DeniedBySandbox { reason: String },
    NeedsApproval { reason: ApprovalReason },
}

#[cfg(test)]
#[path = "decision_tests.rs"]
mod tests;
