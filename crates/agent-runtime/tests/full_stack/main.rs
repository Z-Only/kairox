//! Full-stack integration tests for the LocalRuntime facade.
//!
//! These tests exercise the FULL pipeline:
//!   LocalRuntime → FakeModelClient/ToolCallingModel → ToolRegistry → MemoryStore → EventStore
//!
//! They cover: workspace management, session lifecycle, messaging (text + tool calls),
//! permission decisions, memory protocol, task graph, cancellation, and persistence.
//!
//! Each themed submodule below contains a focused slice of the pipeline; shared
//! fixtures live in `support`.

mod support;

mod cancellation;
mod memory;
mod messaging;
mod persistence;
mod session;
mod streaming;
mod task_graph;
mod workspace;
