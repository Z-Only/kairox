//! StatusBar component — a read-only single-line bar at the bottom of the TUI.

mod context_line;
mod context_overlay;
mod permission_mode;
mod render;
mod state;

#[cfg(test)]
mod tests;

pub use permission_mode::PermissionModeExt;
pub use state::StatusBar;
