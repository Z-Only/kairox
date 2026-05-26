//! `StatusInfo` display helpers for the double-axis approval × sandbox policy.

use crate::components::StatusInfo;

impl StatusInfo {
    /// Approval-axis label (e.g. `on_request`); empty when unset.
    pub fn approval_policy_label(&self) -> &str {
        &self.approval_policy
    }

    /// Sandbox-axis label (e.g. `workspace_write`); empty when unset.
    pub fn sandbox_policy_label(&self) -> &str {
        &self.sandbox_policy
    }
}
