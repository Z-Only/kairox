//! StatusBar component — a read-only single-line bar at the bottom of the TUI.

mod context_line;
mod context_overlay;
mod policy_labels;
mod render;
mod state;
mod types;

#[cfg(test)]
mod tests;

pub use state::StatusBar;
