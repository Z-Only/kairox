//! Approval × Sandbox policy model.
//!
//! Two orthogonal policies drive every tool-execution decision:
//!
//! - [`ApprovalPolicy`] — when to ask the user for approval
//!   (`Never` / `OnRequest` / `Always`)
//! - [`SandboxPolicy`] — what operations the sandbox structurally allows
//!   (`ReadOnly` / `WorkspaceWrite { network_access, writable_roots }` /
//!   `DangerFullAccess`)
//!
//! [`PolicyEngine::decide`] takes a [`PolicyRisk`] and returns a
//! [`PolicyDecision`] (`Allowed`, `DeniedBySandbox`, or `NeedsApproval`).
//!
//! See `docs/superpowers/specs/2026-05-26-permission-sandbox-approval-design.md`
//! for the full decision matrix.

mod approval;
mod decision;
mod effect;
mod engine;
mod sandbox;

pub use approval::ApprovalPolicy;
pub use decision::{ApprovalReason, PolicyDecision};
pub use effect::{PolicyEffect, PolicyRisk};
pub use engine::PolicyEngine;
pub use sandbox::SandboxPolicy;

/// Storage helper: pick the legacy `permission_mode` string for a given
/// `(approval, sandbox)` pair. The DB column is retained one release cycle
/// per spec `2026-05-26-permission-sandbox-approval-design.md` §5.2; writers
/// must keep populating it until the next migration drops the column.
pub fn legacy_mode_string_for(approval: ApprovalPolicy, sandbox: &SandboxPolicy) -> &'static str {
    match (approval, sandbox) {
        (ApprovalPolicy::Never, SandboxPolicy::ReadOnly) => "read_only",
        (ApprovalPolicy::Always, SandboxPolicy::WorkspaceWrite { .. }) => "suggest",
        (ApprovalPolicy::OnRequest, SandboxPolicy::WorkspaceWrite { .. }) => "agent",
        (ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess) => "autonomous",
        (ApprovalPolicy::OnRequest, SandboxPolicy::DangerFullAccess) => "autonomous",
        (ApprovalPolicy::Always, SandboxPolicy::DangerFullAccess) => "suggest",
        (ApprovalPolicy::Always, SandboxPolicy::ReadOnly) => "read_only",
        (ApprovalPolicy::OnRequest, SandboxPolicy::ReadOnly) => "read_only",
        (ApprovalPolicy::Never, SandboxPolicy::WorkspaceWrite { .. }) => "agent",
    }
}

/// Config helper: parse the legacy `permission_mode` string into a canonical
/// `(approval, sandbox)` pair. Used to keep accepting the old enum strings on
/// the agent-settings frontmatter boundary while the rest of the system speaks
/// the double-axis API directly. Returns `None` for unknown strings.
pub fn parse_legacy_mode(s: &str) -> Option<(ApprovalPolicy, SandboxPolicy)> {
    let ws = SandboxPolicy::WorkspaceWrite {
        network_access: false,
        writable_roots: vec![],
    };
    match s {
        "read_only" => Some((ApprovalPolicy::Never, SandboxPolicy::ReadOnly)),
        "suggest" => Some((ApprovalPolicy::Always, ws)),
        "agent" | "workspace_write" => Some((ApprovalPolicy::OnRequest, ws)),
        "autonomous" | "danger_full_access" => {
            Some((ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess))
        }
        "interactive" => Some((ApprovalPolicy::OnRequest, ws)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_mode_string_covers_canonical_pairs() {
        let ws = SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        };
        assert_eq!(
            legacy_mode_string_for(ApprovalPolicy::Never, &SandboxPolicy::ReadOnly),
            "read_only"
        );
        assert_eq!(
            legacy_mode_string_for(ApprovalPolicy::Always, &ws),
            "suggest"
        );
        assert_eq!(
            legacy_mode_string_for(ApprovalPolicy::OnRequest, &ws),
            "agent"
        );
        assert_eq!(
            legacy_mode_string_for(ApprovalPolicy::Never, &SandboxPolicy::DangerFullAccess),
            "autonomous"
        );
    }

    #[test]
    fn parse_legacy_mode_round_trips_canonical_strings() {
        let ws = SandboxPolicy::WorkspaceWrite {
            network_access: false,
            writable_roots: vec![],
        };
        assert_eq!(
            parse_legacy_mode("read_only"),
            Some((ApprovalPolicy::Never, SandboxPolicy::ReadOnly))
        );
        assert_eq!(
            parse_legacy_mode("suggest"),
            Some((ApprovalPolicy::Always, ws.clone()))
        );
        assert_eq!(
            parse_legacy_mode("agent"),
            Some((ApprovalPolicy::OnRequest, ws.clone()))
        );
        assert_eq!(
            parse_legacy_mode("workspace_write"),
            Some((ApprovalPolicy::OnRequest, ws.clone()))
        );
        assert_eq!(
            parse_legacy_mode("autonomous"),
            Some((ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess))
        );
        assert_eq!(
            parse_legacy_mode("interactive"),
            Some((ApprovalPolicy::OnRequest, ws))
        );
        assert_eq!(parse_legacy_mode("bogus"), None);
    }
}
