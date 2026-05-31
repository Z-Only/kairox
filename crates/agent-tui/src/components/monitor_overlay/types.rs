//! Data types for the monitor overlay.

/// Snapshot of a single active monitor, mirrored from `agent_tools::MonitorInfo`
/// so the overlay can be tested without a real `MonitorRegistry`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MonitorEntry {
    pub monitor_id: String,
    pub description: String,
    pub command: String,
    pub persistent: bool,
    pub timeout_ms: u64,
}
