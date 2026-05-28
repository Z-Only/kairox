//! Data types for the permission modal — history entry struct and constants
//! used across the modal submodules.

use crate::components::PermissionRequest;

/// Maximum number of permission history entries retained in the modal.
pub(super) const MAX_HISTORY: usize = 6;

/// A resolved permission request with its approval outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct PermissionHistoryEntry {
    pub request: PermissionRequest,
    pub approved: bool,
}
