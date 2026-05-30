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
