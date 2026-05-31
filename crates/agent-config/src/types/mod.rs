//! Configuration domain types, split into cohesive submodules.
//!
//! Every public item is re-exported here so existing import paths such as
//! `crate::types::Foo` and `agent_config::Foo` (via `lib.rs`) resolve
//! unchanged.

mod config;
mod context;
mod hooks;
mod lsp;
mod mcp;
mod profile;

pub use config::*;
pub use context::*;
pub use hooks::*;
pub use lsp::*;
pub use mcp::*;
pub use profile::*;

// Crate-internal items consumed by sibling modules (e.g. `loader`) through the
// `pub(crate) use types::{..}` re-export in `lib.rs`.
pub(crate) use hooks::HookConfigToml;

/// Serde `default` helper shared by profile, MCP, and hook config types.
pub(crate) fn default_true() -> bool {
    true
}
