//! Local runtime facade.
//!
//! This module composes the [`LocalRuntime`] type, the [`ExecutionMode`] enum,
//! and the supporting `compact_session` inherent method. The implementation is
//! split across focused submodules:
//!
//! - [`execution_mode`] — the [`ExecutionMode`] enum
//! - [`local_runtime`] — the [`LocalRuntime`] struct, its constructor, and
//!   accessor methods
//! - [`compaction_handler`] — the [`LocalRuntime::compact_session`] inherent
//!   method
//! - [`facade_session_ops`] (loaded via `#[path]` from the sibling
//!   `facade_session_ops.rs`) — the `SessionFacade` implementation
//!
//! Other facade traits (`SkillsFacade`, `McpFacade`, …) are implemented in
//! the sibling `facade_*` modules at the crate root.

use agent_core::AppFacade;
use agent_store::EventStore;

mod compaction_handler;
mod execution_mode;
mod local_runtime;

#[path = "facade_session_ops.rs"]
mod facade_session_ops;

pub use execution_mode::ExecutionMode;
pub use local_runtime::LocalRuntime;

impl<S, M> AppFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
}

#[cfg(test)]
mod tests;
