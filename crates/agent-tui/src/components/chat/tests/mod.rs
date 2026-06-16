//! Chat panel unit tests, grouped by behavior area.
//!
//! The chat panel surface mixes composer input, attachment plumbing,
//! permission prompts, render helpers, and the local message queue.
//! Splitting the suite by theme keeps each file focused and well under
//! the 300-line guideline used by the broader TUI test split.

mod common;

mod attachments;
mod effects;
mod input;
mod permissions;
mod queue;
mod render;
mod task_confirmation;
