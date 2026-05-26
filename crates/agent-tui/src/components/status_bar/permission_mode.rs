//! Permission-mode extension trait and `StatusInfo` display helpers.

use crate::components::StatusInfo;

// ---------------------------------------------------------------------------
// PermissionMode extension trait
// ---------------------------------------------------------------------------

/// Extension trait for [`agent_tools::PermissionMode`] to provide display labels.
///
/// We cannot add inherent methods to a foreign type, so we use a trait instead.
pub trait PermissionModeExt {
    /// Return a static string label for the permission mode.
    fn as_str(&self) -> &'static str;

    /// Return the next permission mode in the cycle order.
    ///
    /// Order: ReadOnly → Suggest → Agent → Autonomous → Interactive → ReadOnly.
    fn next(&self) -> agent_tools::PermissionMode;
}

impl PermissionModeExt for agent_tools::PermissionMode {
    fn as_str(&self) -> &'static str {
        match self {
            agent_tools::PermissionMode::ReadOnly => "readonly",
            agent_tools::PermissionMode::Suggest => "suggest",
            agent_tools::PermissionMode::Agent => "agent",
            agent_tools::PermissionMode::Autonomous => "autonomous",
            agent_tools::PermissionMode::Interactive => "interactive",
        }
    }

    fn next(&self) -> agent_tools::PermissionMode {
        match self {
            agent_tools::PermissionMode::ReadOnly => agent_tools::PermissionMode::Suggest,
            agent_tools::PermissionMode::Suggest => agent_tools::PermissionMode::Agent,
            agent_tools::PermissionMode::Agent => agent_tools::PermissionMode::Autonomous,
            agent_tools::PermissionMode::Autonomous => agent_tools::PermissionMode::Interactive,
            agent_tools::PermissionMode::Interactive => agent_tools::PermissionMode::ReadOnly,
        }
    }
}

// ---------------------------------------------------------------------------
// StatusInfo helpers
// ---------------------------------------------------------------------------

impl StatusInfo {
    /// Return a human-readable label for the stored permission mode string.
    ///
    /// Since `permission_mode` is already a `String` set via
    /// `PermissionMode::as_str()`, we simply return it as-is.
    pub fn permission_mode_label(&self) -> &str {
        &self.permission_mode
    }

    /// Approval-axis label (e.g. `on_request`); empty when unset.
    pub fn approval_policy_label(&self) -> &str {
        &self.approval_policy
    }

    /// Sandbox-axis label (e.g. `workspace_write`); empty when unset.
    pub fn sandbox_policy_label(&self) -> &str {
        &self.sandbox_policy
    }
}
