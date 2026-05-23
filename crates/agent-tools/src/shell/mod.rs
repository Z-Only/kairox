//! Shell tool: command classification, parsing, sandboxed execution.
//!
//! The submodules in this directory are organized as:
//!
//! - [`risk`] — [`CommandRisk`] and [`classify_command`] decide read/write/
//!   destructive risk for a given program/argument pair.
//! - [`parse`] — [`parse_command`] tokenizes a shell command string while
//!   honoring single/double quotes and backslash escapes.
//! - [`sandbox`] — env-clearing/allow-list, timeout/output limits, and the
//!   byte truncation helper used to bound captured stdio.
//! - [`exec`] — [`ShellExecTool`] and its [`crate::registry::Tool`] impl.

pub mod exec;
pub mod parse;
pub mod risk;
pub mod sandbox;

#[cfg(test)]
mod tests;

pub use exec::ShellExecTool;
pub use parse::parse_command;
pub use risk::{classify_command, CommandRisk};

// Tool IDs are exposed at `crate::shell::*` because sibling tool modules
// (`patch`, `search`) reference them directly. Keeping them here preserves
// the previous public path.
pub const SHELL_TOOL_ID: &str = "shell.exec";
pub const PATCH_TOOL_ID: &str = "patch.apply";
pub const SEARCH_TOOL_ID: &str = "search.ripgrep";
