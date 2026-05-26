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
