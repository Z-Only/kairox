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
mod tests {
    use super::*;

    #[test]
    fn reason_display_snake_case() {
        assert_eq!(ApprovalReason::PolicyAlways.to_string(), "policy_always");
        assert_eq!(
            ApprovalReason::UntrustedMcpServer.to_string(),
            "untrusted_mcp_server"
        );
    }

    #[test]
    fn reason_serde_roundtrip() {
        for r in [
            ApprovalReason::SandboxRejected,
            ApprovalReason::PolicyAlways,
            ApprovalReason::DestructiveEffect,
            ApprovalReason::UnknownCommand,
            ApprovalReason::NetworkRequest,
            ApprovalReason::UntrustedMcpServer,
        ] {
            let s = serde_json::to_string(&r).unwrap();
            let back: ApprovalReason = serde_json::from_str(&s).unwrap();
            assert_eq!(back, r);
        }
    }
}
