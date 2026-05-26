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

use crate::permission::PermissionMode;

/// Migration helper for callers still passing the legacy [`PermissionMode`].
/// Maps each legacy value to the (approval, sandbox) pair it now expands to.
///
/// | `PermissionMode` | `ApprovalPolicy` | `SandboxPolicy`                                              |
/// |------------------|------------------|--------------------------------------------------------------|
/// | `ReadOnly`       | `Never`          | `ReadOnly`                                                   |
/// | `Suggest`        | `Always`         | `WorkspaceWrite { network=false, roots=[] }`                 |
/// | `Agent`          | `OnRequest`      | `WorkspaceWrite { network=false, roots=[] }`                 |
/// | `Autonomous`     | `Never`          | `DangerFullAccess`                                           |
/// | `Interactive`    | `OnRequest`      | `WorkspaceWrite { network=false, roots=[] }`                 |
impl From<PermissionMode> for (ApprovalPolicy, SandboxPolicy) {
    fn from(mode: PermissionMode) -> Self {
        match mode {
            PermissionMode::ReadOnly => (ApprovalPolicy::Never, SandboxPolicy::ReadOnly),
            PermissionMode::Suggest => (
                ApprovalPolicy::Always,
                SandboxPolicy::WorkspaceWrite {
                    network_access: false,
                    writable_roots: vec![],
                },
            ),
            PermissionMode::Agent => (
                ApprovalPolicy::OnRequest,
                SandboxPolicy::WorkspaceWrite {
                    network_access: false,
                    writable_roots: vec![],
                },
            ),
            PermissionMode::Autonomous => (ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess),
            PermissionMode::Interactive => (
                ApprovalPolicy::OnRequest,
                SandboxPolicy::WorkspaceWrite {
                    network_access: false,
                    writable_roots: vec![],
                },
            ),
        }
    }
}

/// Inverse helper: pick the closest legacy [`PermissionMode`] for a given
/// `(approval, sandbox)` pair. Used by storage layers that still serialize
/// to the legacy column during the transition window.
pub fn legacy_mode_for(approval: ApprovalPolicy, sandbox: &SandboxPolicy) -> PermissionMode {
    match (approval, sandbox) {
        (ApprovalPolicy::Never, SandboxPolicy::ReadOnly) => PermissionMode::ReadOnly,
        (ApprovalPolicy::Always, SandboxPolicy::WorkspaceWrite { .. }) => PermissionMode::Suggest,
        (ApprovalPolicy::OnRequest, SandboxPolicy::WorkspaceWrite { .. }) => PermissionMode::Agent,
        (ApprovalPolicy::Never, SandboxPolicy::DangerFullAccess) => PermissionMode::Autonomous,
        (ApprovalPolicy::OnRequest, SandboxPolicy::DangerFullAccess) => PermissionMode::Autonomous,
        (ApprovalPolicy::Always, SandboxPolicy::DangerFullAccess) => PermissionMode::Suggest,
        (ApprovalPolicy::Always, SandboxPolicy::ReadOnly) => PermissionMode::ReadOnly,
        (ApprovalPolicy::OnRequest, SandboxPolicy::ReadOnly) => PermissionMode::ReadOnly,
        (ApprovalPolicy::Never, SandboxPolicy::WorkspaceWrite { .. }) => PermissionMode::Agent,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_mode_roundtrips_for_canonical_pairs() {
        for mode in [
            PermissionMode::ReadOnly,
            PermissionMode::Suggest,
            PermissionMode::Agent,
            PermissionMode::Autonomous,
            PermissionMode::Interactive,
        ] {
            let (a, s): (ApprovalPolicy, SandboxPolicy) = mode.into();
            // Interactive collapses to Agent (same pair). Other four roundtrip.
            let back = legacy_mode_for(a, &s);
            if mode == PermissionMode::Interactive {
                assert_eq!(back, PermissionMode::Agent);
            } else {
                assert_eq!(back, mode);
            }
        }
    }
}
