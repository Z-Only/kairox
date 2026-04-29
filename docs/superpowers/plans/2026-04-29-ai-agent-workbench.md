# AI Agent Workbench Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a local-first, cross-platform AI agent workbench with a shared Rust core, Ratatui TUI, Tauri/Vue GUI, model routing, tools, permissions, memory, trace, audit, MCP, manifests, and lightweight multi-agent orchestration.

**Architecture:** Implement an event-sourced Rust runtime behind a stable `AppFacade` boundary. The TUI and GUI call the same facade commands, queries, and subscriptions; storage, tools, model adapters, permissions, memory, and orchestration remain UI-agnostic.

**Tech Stack:** Rust workspace, Tokio, serde, thiserror, async-trait, sqlx SQLite, ratatui, crossterm, Tauri 2, Vue 3, TypeScript, Vitest, Vite, toml, schemars, tempfile, insta.

---

## Scope Split

The source spec spans several independent subsystems. Implement it as a milestone plan where each task produces working, testable software and can be committed independently:

- M0: Workspace foundation, domain events, store, fake model, facade skeleton.
- M1: TUI single-agent loop over the shared facade.
- M2: Models, tools, and permissions.
- M3: Tauri/Vue GUI shell and command bridge.
- M4: Memory, context assembly, MCP, and manifests.
- M5: Lightweight multi-agent runtime.
- M6: Optional account boundary and sync-ready interfaces.

## File Structure

All paths are relative to `/Users/chanyu/AIProjects/kairox`.

- `Cargo.toml`: Rust workspace members and shared dependency versions.
- `.gitignore`: generated Rust, Node, SQLite, and Tauri build artifacts.
- `crates/agent-core`: domain types, events, facade traits, projections, privacy classifications, error types.
- `crates/agent-runtime`: session runtime, agent loop, scheduler, task graph, orchestration.
- `crates/agent-models`: provider traits, fake provider, OpenAI-compatible adapter, Ollama adapter, model profiles.
- `crates/agent-tools`: tool registry, filesystem/search/git/shell/patch tools, MCP adapter boundary.
- `crates/agent-memory`: user/workspace/session memory and context assembly.
- `crates/agent-store`: SQLite persistence, migrations, event append/replay, local config.
- `crates/agent-tui`: Ratatui application using only `agent-core` facade types and `agent-runtime` composition.
- `apps/agent-gui`: Tauri/Vue GUI. Rust commands live in `apps/agent-gui/src-tauri`; Vue frontend lives in `apps/agent-gui/src`.
- `tests/e2e`: black-box Rust integration tests using fake providers and temporary workspaces.
- `fixtures`: fixture workspaces, model streams, MCP server manifests, skill/plugin manifests.
- `docs/dev`: local development notes for commands, architecture decisions, and release checks.

## Cross-Cutting Rules

- Keep core crates UI-agnostic. `agent-core`, `agent-runtime`, `agent-models`, `agent-tools`, `agent-memory`, and `agent-store` must not depend on Ratatui, Tauri, Vue, or crossterm.
- All mutating runtime activity must emit append-only events with schema version, workspace id, session id, timestamp, source agent id, and privacy classification.
- Tool execution must flow through `PermissionEngine` before side effects.
- Durable memory writes must be proposed through events before acceptance.
- Every task ends with a focused commit.

---

### Task 1: Rust Workspace Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `.gitignore`
- Create: `crates/agent-core/Cargo.toml`
- Create: `crates/agent-core/src/lib.rs`
- Create: `crates/agent-runtime/Cargo.toml`
- Create: `crates/agent-runtime/src/lib.rs`
- Create: `crates/agent-models/Cargo.toml`
- Create: `crates/agent-models/src/lib.rs`
- Create: `crates/agent-tools/Cargo.toml`
- Create: `crates/agent-tools/src/lib.rs`
- Create: `crates/agent-memory/Cargo.toml`
- Create: `crates/agent-memory/src/lib.rs`
- Create: `crates/agent-store/Cargo.toml`
- Create: `crates/agent-store/src/lib.rs`
- Create: `crates/agent-tui/Cargo.toml`
- Create: `crates/agent-tui/src/main.rs`

- [ ] **Step 1: Write the failing workspace smoke test**

Create `crates/agent-core/src/lib.rs` with a temporary exported version constant:

```rust
pub const CORE_CRATE_NAME: &str = "agent-core";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_core_crate_name() {
        assert_eq!(CORE_CRATE_NAME, "agent-core");
    }
}
```

- [ ] **Step 2: Run test before the workspace exists**

Run: `cargo test -p agent-core exposes_core_crate_name`

Expected: FAIL because the root workspace and crate manifests do not exist yet.

- [ ] **Step 3: Create the workspace manifests**

Create root `Cargo.toml`:

```toml
[workspace]
members = [
  "crates/agent-core",
  "crates/agent-runtime",
  "crates/agent-models",
  "crates/agent-tools",
  "crates/agent-memory",
  "crates/agent-store",
  "crates/agent-tui",
]
resolver = "2"

[workspace.package]
edition = "2021"
license = "Apache-2.0"
version = "0.1.0"

[workspace.dependencies]
anyhow = "1"
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde", "clock"] }
futures = "0.3"
insta = { version = "1", features = ["yaml"] }
ratatui = "0.29"
schemars = { version = "0.8", features = ["chrono", "uuid1"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
sqlx = { version = "0.8", features = ["runtime-tokio-rustls", "sqlite", "chrono", "uuid", "json"] }
tempfile = "3"
thiserror = "2"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "sync", "time", "process"] }
tokio-stream = "0.1"
toml = "0.8"
uuid = { version = "1", features = ["serde", "v4"] }
```

Create `.gitignore`:

```gitignore
/target/
/node_modules/
/dist/
/.DS_Store
**/.DS_Store
*.db
*.db-shm
*.db-wal
.env
.env.*
!.env.example
apps/agent-gui/src-tauri/target/
apps/agent-gui/node_modules/
apps/agent-gui/dist/
```

Create each crate manifest using this pattern, replacing the crate name:

```toml
[package]
name = "agent-core"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
chrono.workspace = true
schemars.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
uuid.workspace = true
```

Use these crate-specific dependencies:

```toml
# crates/agent-runtime/Cargo.toml
[dependencies]
agent-core = { path = "../agent-core" }
agent-models = { path = "../agent-models" }
agent-store = { path = "../agent-store" }
agent-tools = { path = "../agent-tools" }
anyhow.workspace = true
async-trait.workspace = true
futures.workspace = true
tokio.workspace = true
tokio-stream.workspace = true
```

```toml
# crates/agent-models/Cargo.toml
[dependencies]
agent-core = { path = "../agent-core" }
async-trait.workspace = true
futures.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tokio-stream.workspace = true
```

```toml
# crates/agent-tools/Cargo.toml
[dependencies]
agent-core = { path = "../agent-core" }
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
```

```toml
# crates/agent-memory/Cargo.toml
[dependencies]
agent-core = { path = "../agent-core" }
serde.workspace = true
thiserror.workspace = true
```

```toml
# crates/agent-store/Cargo.toml
[dependencies]
agent-core = { path = "../agent-core" }
async-trait.workspace = true
serde_json.workspace = true
sqlx.workspace = true
thiserror.workspace = true
tokio.workspace = true
uuid.workspace = true
```

```toml
# crates/agent-tui/Cargo.toml
[package]
name = "agent-tui"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
agent-core = { path = "../agent-core" }
agent-runtime = { path = "../agent-runtime" }
anyhow.workspace = true
ratatui.workspace = true
tokio.workspace = true
```

Create minimal library files:

```rust
// crates/agent-runtime/src/lib.rs
pub const RUNTIME_CRATE_NAME: &str = "agent-runtime";
```

```rust
// crates/agent-models/src/lib.rs
pub const MODELS_CRATE_NAME: &str = "agent-models";
```

```rust
// crates/agent-tools/src/lib.rs
pub const TOOLS_CRATE_NAME: &str = "agent-tools";
```

```rust
// crates/agent-memory/src/lib.rs
pub const MEMORY_CRATE_NAME: &str = "agent-memory";
```

```rust
// crates/agent-store/src/lib.rs
pub const STORE_CRATE_NAME: &str = "agent-store";
```

Create `crates/agent-tui/src/main.rs`:

```rust
fn main() {
    println!("agent-tui");
}
```

- [ ] **Step 4: Verify the scaffold**

Run: `cargo test --workspace`

Expected: PASS with the `agent-core` smoke test.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml .gitignore crates
git commit -m "chore: scaffold rust workspace"
```

---

### Task 2: Core IDs, Events, and Projections

**Files:**
- Create: `crates/agent-core/src/ids.rs`
- Create: `crates/agent-core/src/events.rs`
- Create: `crates/agent-core/src/projection.rs`
- Create: `crates/agent-core/src/error.rs`
- Modify: `crates/agent-core/src/lib.rs`

- [ ] **Step 1: Write failing event serialization tests**

Create `crates/agent-core/src/events.rs` with the tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::{AgentId, SessionId, WorkspaceId};
    use chrono::TimeZone;

    #[test]
    fn serializes_user_message_event_with_required_envelope_fields() {
        let event = DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::UserMessageAdded {
                message_id: "msg-user-1".into(),
                content: "explain the repo".into(),
            },
        )
        .with_timestamp(chrono::Utc.with_ymd_and_hms(2026, 4, 29, 2, 0, 0).unwrap());

        let json = serde_json::to_value(&event).unwrap();

        assert_eq!(json["schema_version"], 1);
        assert_eq!(json["event_type"], "UserMessageAdded");
        assert_eq!(json["privacy"], "full_trace");
        assert_eq!(json["payload"]["content"], "explain the repo");
        assert!(json["workspace_id"].as_str().unwrap().starts_with("wrk_"));
        assert!(json["session_id"].as_str().unwrap().starts_with("ses_"));
    }
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test -p agent-core serializes_user_message_event_with_required_envelope_fields`

Expected: FAIL with unresolved `DomainEvent`, `PrivacyClassification`, `EventPayload`, and ID types.

- [ ] **Step 3: Implement ID and event types**

Create `crates/agent-core/src/ids.rs`:

```rust
use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! prefixed_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, Uuid::new_v4().simple()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

prefixed_id!(WorkspaceId, "wrk");
prefixed_id!(SessionId, "ses");
prefixed_id!(TaskId, "tsk");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    pub fn system() -> Self {
        Self("agent_system".into())
    }

    pub fn planner() -> Self {
        Self("agent_planner".into())
    }

    pub fn worker(worker_name: impl Into<String>) -> Self {
        Self(format!("agent_worker_{}", worker_name.into()))
    }

    pub fn reviewer() -> Self {
        Self("agent_reviewer".into())
    }
}
```

Create `crates/agent-core/src/events.rs`:

```rust
use crate::ids::{AgentId, SessionId, TaskId, WorkspaceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyClassification {
    MinimalTrace,
    FullTrace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "PascalCase")]
pub enum EventPayload {
    WorkspaceOpened { path: String },
    UserMessageAdded { message_id: String, content: String },
    AgentTaskCreated { task_id: TaskId, title: String },
    AgentTaskStarted { task_id: TaskId },
    ContextAssembled { token_estimate: usize, sources: Vec<String> },
    ModelRequestStarted { model_profile: String, model_id: String },
    ModelTokenDelta { delta: String },
    ModelToolCallRequested { tool_call_id: String, tool_id: String },
    PermissionRequested { request_id: String, tool_id: String, preview: String },
    PermissionGranted { request_id: String },
    PermissionDenied { request_id: String, reason: String },
    ToolInvocationStarted { invocation_id: String, tool_id: String },
    ToolInvocationCompleted { invocation_id: String, output_preview: String },
    ToolInvocationFailed { invocation_id: String, error: String },
    FilePatchProposed { patch_id: String, diff: String },
    FilePatchApplied { patch_id: String },
    MemoryProposed { memory_id: String, content: String },
    MemoryAccepted { memory_id: String },
    MemoryRejected { memory_id: String, reason: String },
    ReviewerFindingAdded { finding_id: String, severity: String, message: String },
    AssistantMessageCompleted { message_id: String, content: String },
    AgentTaskCompleted { task_id: TaskId },
    AgentTaskFailed { task_id: TaskId, error: String },
    SessionCancelled { reason: String },
}

impl EventPayload {
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::WorkspaceOpened { .. } => "WorkspaceOpened",
            Self::UserMessageAdded { .. } => "UserMessageAdded",
            Self::AgentTaskCreated { .. } => "AgentTaskCreated",
            Self::AgentTaskStarted { .. } => "AgentTaskStarted",
            Self::ContextAssembled { .. } => "ContextAssembled",
            Self::ModelRequestStarted { .. } => "ModelRequestStarted",
            Self::ModelTokenDelta { .. } => "ModelTokenDelta",
            Self::ModelToolCallRequested { .. } => "ModelToolCallRequested",
            Self::PermissionRequested { .. } => "PermissionRequested",
            Self::PermissionGranted { .. } => "PermissionGranted",
            Self::PermissionDenied { .. } => "PermissionDenied",
            Self::ToolInvocationStarted { .. } => "ToolInvocationStarted",
            Self::ToolInvocationCompleted { .. } => "ToolInvocationCompleted",
            Self::ToolInvocationFailed { .. } => "ToolInvocationFailed",
            Self::FilePatchProposed { .. } => "FilePatchProposed",
            Self::FilePatchApplied { .. } => "FilePatchApplied",
            Self::MemoryProposed { .. } => "MemoryProposed",
            Self::MemoryAccepted { .. } => "MemoryAccepted",
            Self::MemoryRejected { .. } => "MemoryRejected",
            Self::ReviewerFindingAdded { .. } => "ReviewerFindingAdded",
            Self::AssistantMessageCompleted { .. } => "AssistantMessageCompleted",
            Self::AgentTaskCompleted { .. } => "AgentTaskCompleted",
            Self::AgentTaskFailed { .. } => "AgentTaskFailed",
            Self::SessionCancelled { .. } => "SessionCancelled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DomainEvent {
    pub schema_version: u32,
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub timestamp: DateTime<Utc>,
    pub source_agent_id: AgentId,
    pub privacy: PrivacyClassification,
    pub event_type: &'static str,
    pub payload: EventPayload,
}

impl DomainEvent {
    pub fn new(
        workspace_id: WorkspaceId,
        session_id: SessionId,
        source_agent_id: AgentId,
        privacy: PrivacyClassification,
        payload: EventPayload,
    ) -> Self {
        let event_type = payload.event_type();
        Self {
            schema_version: 1,
            workspace_id,
            session_id,
            timestamp: Utc::now(),
            source_agent_id,
            privacy,
            event_type,
            payload,
        }
    }

    pub fn with_timestamp(mut self, timestamp: DateTime<Utc>) -> Self {
        self.timestamp = timestamp;
        self
    }
}
```

Create `crates/agent-core/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("invalid state: {0}")]
    InvalidState(String),
}
```

Update `crates/agent-core/src/lib.rs`:

```rust
pub mod error;
pub mod events;
pub mod ids;
pub mod projection;

pub use error::CoreError;
pub use events::{DomainEvent, EventPayload, PrivacyClassification};
pub use ids::{AgentId, SessionId, TaskId, WorkspaceId};
```

- [ ] **Step 4: Add projection test and implementation**

Create `crates/agent-core/src/projection.rs`:

```rust
use crate::events::{DomainEvent, EventPayload};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SessionProjection {
    pub messages: Vec<ProjectedMessage>,
    pub task_titles: Vec<String>,
    pub token_stream: String,
    pub cancelled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectedMessage {
    pub role: ProjectedRole,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectedRole {
    User,
    Assistant,
}

impl SessionProjection {
    pub fn apply(&mut self, event: &DomainEvent) {
        match &event.payload {
            EventPayload::UserMessageAdded { content, .. } => self.messages.push(ProjectedMessage {
                role: ProjectedRole::User,
                content: content.clone(),
            }),
            EventPayload::ModelTokenDelta { delta } => self.token_stream.push_str(delta),
            EventPayload::AssistantMessageCompleted { content, .. } => {
                self.messages.push(ProjectedMessage {
                    role: ProjectedRole::Assistant,
                    content: content.clone(),
                });
                self.token_stream.clear();
            }
            EventPayload::AgentTaskCreated { title, .. } => self.task_titles.push(title.clone()),
            EventPayload::SessionCancelled { .. } => self.cancelled = true,
            _ => {}
        }
    }

    pub fn from_events(events: &[DomainEvent]) -> Self {
        let mut projection = Self::default();
        for event in events {
            projection.apply(event);
        }
        projection
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AgentId, EventPayload, PrivacyClassification, SessionId, WorkspaceId};

    #[test]
    fn projects_user_and_assistant_messages() {
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();
        let events = vec![
            DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::UserMessageAdded {
                    message_id: "m1".into(),
                    content: "hello".into(),
                },
            ),
            DomainEvent::new(
                workspace_id,
                session_id,
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::AssistantMessageCompleted {
                    message_id: "m2".into(),
                    content: "hi".into(),
                },
            ),
        ];

        let projection = SessionProjection::from_events(&events);

        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].role, ProjectedRole::User);
        assert_eq!(projection.messages[1].content, "hi");
    }
}
```

Run: `cargo test -p agent-core`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-core
git commit -m "feat(core): add event schema and session projection"
```

---

### Task 3: App Facade Traits

**Files:**
- Create: `crates/agent-core/src/facade.rs`
- Modify: `crates/agent-core/src/lib.rs`
- Modify: `crates/agent-core/Cargo.toml`

- [ ] **Step 1: Add facade dependency**

Modify `crates/agent-core/Cargo.toml`:

```toml
[dependencies]
async-trait.workspace = true
chrono.workspace = true
futures.workspace = true
schemars.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
uuid.workspace = true
```

- [ ] **Step 2: Write facade compile test**

Create `crates/agent-core/src/facade.rs`:

```rust
use crate::{DomainEvent, SessionId, WorkspaceId};
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub workspace_id: WorkspaceId,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StartSessionRequest {
    pub workspace_id: WorkspaceId,
    pub model_profile: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendMessageRequest {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PermissionDecision {
    pub request_id: String,
    pub approve: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceEntry {
    pub event: DomainEvent,
}

#[async_trait]
pub trait AppFacade: Send + Sync {
    async fn open_workspace(&self, path: String) -> crate::Result<WorkspaceInfo>;
    async fn start_session(&self, request: StartSessionRequest) -> crate::Result<SessionId>;
    async fn send_message(&self, request: SendMessageRequest) -> crate::Result<()>;
    async fn decide_permission(&self, decision: PermissionDecision) -> crate::Result<()>;
    async fn cancel_session(&self, workspace_id: WorkspaceId, session_id: SessionId) -> crate::Result<()>;
    async fn get_session_projection(&self, session_id: SessionId) -> crate::Result<crate::projection::SessionProjection>;
    async fn get_trace(&self, session_id: SessionId) -> crate::Result<Vec<TraceEntry>>;
    fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, DomainEvent>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_facade_is_object_safe(_: &dyn AppFacade) {}

    struct NoopFacade;

    #[async_trait]
    impl AppFacade for NoopFacade {
        async fn open_workspace(&self, path: String) -> crate::Result<WorkspaceInfo> {
            Ok(WorkspaceInfo { workspace_id: WorkspaceId::new(), path })
        }

        async fn start_session(&self, request: StartSessionRequest) -> crate::Result<SessionId> {
            let _ = request;
            Ok(SessionId::new())
        }

        async fn send_message(&self, request: SendMessageRequest) -> crate::Result<()> {
            let _ = request;
            Ok(())
        }

        async fn decide_permission(&self, decision: PermissionDecision) -> crate::Result<()> {
            let _ = decision;
            Ok(())
        }

        async fn cancel_session(&self, workspace_id: WorkspaceId, session_id: SessionId) -> crate::Result<()> {
            let _ = (workspace_id, session_id);
            Ok(())
        }

        async fn get_session_projection(&self, session_id: SessionId) -> crate::Result<crate::projection::SessionProjection> {
            let _ = session_id;
            Ok(crate::projection::SessionProjection::default())
        }

        async fn get_trace(&self, session_id: SessionId) -> crate::Result<Vec<TraceEntry>> {
            let _ = session_id;
            Ok(Vec::new())
        }

        fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, DomainEvent> {
            let _ = session_id;
            Box::pin(futures::stream::empty())
        }
    }

    #[test]
    fn facade_is_object_safe() {
        let facade = NoopFacade;
        assert_facade_is_object_safe(&facade);
    }
}
```

- [ ] **Step 3: Export facade and result alias**

Update `crates/agent-core/src/lib.rs`:

```rust
pub mod error;
pub mod events;
pub mod facade;
pub mod ids;
pub mod projection;

pub use error::CoreError;
pub use events::{DomainEvent, EventPayload, PrivacyClassification};
pub use facade::{
    AppFacade, PermissionDecision, SendMessageRequest, StartSessionRequest, TraceEntry, WorkspaceInfo,
};
pub use ids::{AgentId, SessionId, TaskId, WorkspaceId};

pub type Result<T> = std::result::Result<T, CoreError>;
```

- [ ] **Step 4: Verify**

Run: `cargo test -p agent-core facade_is_object_safe`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-core
git commit -m "feat(core): define app facade boundary"
```

---

### Task 4: SQLite Event Store

**Files:**
- Create: `crates/agent-store/migrations/0001_events.sql`
- Create: `crates/agent-store/src/event_store.rs`
- Modify: `crates/agent-store/src/lib.rs`

- [ ] **Step 1: Write store round-trip test**

Create `crates/agent-store/src/event_store.rs`:

```rust
use agent_core::{DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId, AgentId};
use async_trait::async_trait;

#[async_trait]
pub trait EventStore: Send + Sync {
    async fn append(&self, event: &DomainEvent) -> crate::Result<()>;
    async fn load_session(&self, session_id: &SessionId) -> crate::Result<Vec<DomainEvent>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn appends_and_replays_session_events_in_order() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();

        let first = DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::UserMessageAdded {
                message_id: "m1".into(),
                content: "hello".into(),
            },
        );
        let second = DomainEvent::new(
            workspace_id,
            session_id.clone(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            EventPayload::AssistantMessageCompleted {
                message_id: "m2".into(),
                content: "hi".into(),
            },
        );

        store.append(&first).await.unwrap();
        store.append(&second).await.unwrap();

        let replayed = store.load_session(&session_id).await.unwrap();
        assert_eq!(replayed.len(), 2);
        assert_eq!(replayed[0].event_type, "UserMessageAdded");
        assert_eq!(replayed[1].event_type, "AssistantMessageCompleted");
    }
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test -p agent-store appends_and_replays_session_events_in_order`

Expected: FAIL with unresolved `SqliteEventStore`.

- [ ] **Step 3: Implement migration and store**

Create `crates/agent-store/migrations/0001_events.sql`:

```sql
CREATE TABLE IF NOT EXISTS events (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    schema_version INTEGER NOT NULL,
    workspace_id TEXT NOT NULL,
    session_id TEXT NOT NULL,
    timestamp TEXT NOT NULL,
    source_agent_id TEXT NOT NULL,
    privacy TEXT NOT NULL,
    event_type TEXT NOT NULL,
    payload_json TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_events_session_id_id ON events(session_id, id);
```

Append the implementation below the test imports in `crates/agent-store/src/event_store.rs`:

```rust
use sqlx::{sqlite::SqlitePoolOptions, Row, SqlitePool};

#[derive(Clone)]
pub struct SqliteEventStore {
    pool: SqlitePool,
}

impl SqliteEventStore {
    pub async fn in_memory() -> crate::Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> crate::Result<()> {
        sqlx::query(include_str!("../migrations/0001_events.sql"))
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl EventStore for SqliteEventStore {
    async fn append(&self, event: &DomainEvent) -> crate::Result<()> {
        let payload_json = serde_json::to_string(event)?;
        sqlx::query(
            "INSERT INTO events (schema_version, workspace_id, session_id, timestamp, source_agent_id, privacy, event_type, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        )
        .bind(event.schema_version as i64)
        .bind(event.workspace_id.to_string())
        .bind(event.session_id.to_string())
        .bind(event.timestamp.to_rfc3339())
        .bind(serde_json::to_string(&event.source_agent_id)?)
        .bind(serde_json::to_string(&event.privacy)?)
        .bind(event.event_type)
        .bind(payload_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn load_session(&self, session_id: &SessionId) -> crate::Result<Vec<DomainEvent>> {
        let rows = sqlx::query("SELECT payload_json FROM events WHERE session_id = ?1 ORDER BY id ASC")
            .bind(session_id.to_string())
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter()
            .map(|row| {
                let payload_json: String = row.try_get("payload_json")?;
                let event = serde_json::from_str(&payload_json)?;
                Ok(event)
            })
            .collect()
    }
}
```

Update `crates/agent-store/src/lib.rs`:

```rust
pub mod event_store;

pub use event_store::{EventStore, SqliteEventStore};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;
```

- [ ] **Step 4: Verify**

Run: `cargo test -p agent-store`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-store
git commit -m "feat(store): persist append-only events in sqlite"
```

---

### Task 5: Fake Model Provider and Session Runtime

**Files:**
- Create: `crates/agent-models/src/types.rs`
- Create: `crates/agent-models/src/fake.rs`
- Modify: `crates/agent-models/src/lib.rs`
- Create: `crates/agent-runtime/src/facade_runtime.rs`
- Modify: `crates/agent-runtime/src/lib.rs`

- [ ] **Step 1: Write fake streaming provider test**

Create `crates/agent-models/src/fake.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelClient, ModelEvent, ModelRequest};
    use futures::StreamExt;

    #[tokio::test]
    async fn streams_configured_tokens_then_completion() {
        let client = FakeModelClient::new(vec!["hello".into(), " ".into(), "world".into()]);
        let mut stream = client.stream(ModelRequest::user_text("test", "hi")).await.unwrap();

        let mut seen = Vec::new();
        while let Some(event) = stream.next().await {
            seen.push(event.unwrap());
        }

        assert_eq!(seen, vec![
            ModelEvent::TokenDelta("hello".into()),
            ModelEvent::TokenDelta(" ".into()),
            ModelEvent::TokenDelta("world".into()),
            ModelEvent::Completed { usage: None },
        ]);
    }
}
```

- [ ] **Step 2: Implement model traits and fake provider**

Create `crates/agent-models/src/types.rs`:

```rust
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model_profile: String,
    pub messages: Vec<ModelMessage>,
}

impl ModelRequest {
    pub fn user_text(model_profile: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            model_profile: model_profile.into(),
            messages: vec![ModelMessage {
                role: "user".into(),
                content: content.into(),
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelEvent {
    TokenDelta(String),
    ToolCallRequested { tool_call_id: String, tool_id: String, arguments: serde_json::Value },
    Completed { usage: Option<ModelUsage> },
    Failed { message: String },
}

#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn stream(&self, request: ModelRequest) -> crate::Result<BoxStream<'static, crate::Result<ModelEvent>>>;
}
```

Replace `crates/agent-models/src/fake.rs` with:

```rust
use crate::{ModelClient, ModelEvent, ModelRequest};
use async_trait::async_trait;
use futures::{stream, stream::BoxStream};

#[derive(Debug, Clone)]
pub struct FakeModelClient {
    tokens: Vec<String>,
}

impl FakeModelClient {
    pub fn new(tokens: Vec<String>) -> Self {
        Self { tokens }
    }
}

#[async_trait]
impl ModelClient for FakeModelClient {
    async fn stream(&self, request: ModelRequest) -> crate::Result<BoxStream<'static, crate::Result<ModelEvent>>> {
        let _ = request;
        let mut events: Vec<crate::Result<ModelEvent>> = self
            .tokens
            .iter()
            .cloned()
            .map(ModelEvent::TokenDelta)
            .map(Ok)
            .collect();
        events.push(Ok(ModelEvent::Completed { usage: None }));
        Ok(Box::pin(stream::iter(events)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelEvent, ModelRequest};
    use futures::StreamExt;

    #[tokio::test]
    async fn streams_configured_tokens_then_completion() {
        let client = FakeModelClient::new(vec!["hello".into(), " ".into(), "world".into()]);
        let mut stream = client.stream(ModelRequest::user_text("test", "hi")).await.unwrap();

        let mut seen = Vec::new();
        while let Some(event) = stream.next().await {
            seen.push(event.unwrap());
        }

        assert_eq!(seen, vec![
            ModelEvent::TokenDelta("hello".into()),
            ModelEvent::TokenDelta(" ".into()),
            ModelEvent::TokenDelta("world".into()),
            ModelEvent::Completed { usage: None },
        ]);
    }
}
```

Update `crates/agent-models/src/lib.rs`:

```rust
pub mod fake;
pub mod types;

pub use fake::FakeModelClient;
pub use types::{ModelClient, ModelEvent, ModelMessage, ModelRequest, ModelUsage};

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model request failed: {0}")]
    Request(String),
}

pub type Result<T> = std::result::Result<T, ModelError>;
```

- [ ] **Step 3: Verify model tests**

Run: `cargo test -p agent-models`

Expected: PASS.

- [ ] **Step 4: Write runtime facade test**

Create `crates/agent-runtime/src/facade_runtime.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
    use agent_models::FakeModelClient;
    use agent_store::SqliteEventStore;

    #[tokio::test]
    async fn send_message_records_user_and_assistant_events() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hello".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/workspace".into()).await.unwrap();
        let session_id = runtime.start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        }).await.unwrap();

        runtime.send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hi".into(),
        }).await.unwrap();

        let projection = runtime.get_session_projection(session_id).await.unwrap();
        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].content, "hi");
        assert_eq!(projection.messages[1].content, "hello");
    }
}
```

- [ ] **Step 5: Implement minimal local runtime**

Replace `crates/agent-runtime/src/facade_runtime.rs` with:

```rust
use agent_core::{
    AgentId, AppFacade, DomainEvent, EventPayload, PermissionDecision, PrivacyClassification,
    SendMessageRequest, SessionId, StartSessionRequest, TraceEntry, WorkspaceId, WorkspaceInfo,
};
use agent_models::{ModelClient, ModelEvent, ModelRequest};
use agent_store::EventStore;
use async_trait::async_trait;
use futures::{stream, StreamExt};
use std::sync::Arc;

pub struct LocalRuntime<S, M> {
    store: Arc<S>,
    model: Arc<M>,
}

impl<S, M> LocalRuntime<S, M> {
    pub fn new(store: S, model: M) -> Self {
        Self {
            store: Arc::new(store),
            model: Arc::new(model),
        }
    }
}

#[async_trait]
impl<S, M> AppFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    async fn open_workspace(&self, path: String) -> agent_core::Result<WorkspaceInfo> {
        let workspace_id = WorkspaceId::new();
        let session_id = SessionId::new();
        self.store
            .append(&DomainEvent::new(
                workspace_id.clone(),
                session_id,
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::WorkspaceOpened { path: path.clone() },
            ))
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(WorkspaceInfo { workspace_id, path })
    }

    async fn start_session(&self, request: StartSessionRequest) -> agent_core::Result<SessionId> {
        let session_id = SessionId::new();
        self.store
            .append(&DomainEvent::new(
                request.workspace_id,
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::AgentTaskCreated {
                    task_id: agent_core::TaskId::new(),
                    title: format!("Session using {}", request.model_profile),
                },
            ))
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(session_id)
    }

    async fn send_message(&self, request: SendMessageRequest) -> agent_core::Result<()> {
        self.store
            .append(&DomainEvent::new(
                request.workspace_id.clone(),
                request.session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::UserMessageAdded {
                    message_id: "msg_user_latest".into(),
                    content: request.content.clone(),
                },
            ))
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;

        let mut stream = self
            .model
            .stream(ModelRequest::user_text("fake", request.content))
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let mut assistant = String::new();
        while let Some(event) = stream.next().await {
            match event.map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))? {
                ModelEvent::TokenDelta(delta) => {
                    assistant.push_str(&delta);
                    self.store
                        .append(&DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::ModelTokenDelta { delta },
                        ))
                        .await
                        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
                }
                ModelEvent::Completed { .. } => {
                    self.store
                        .append(&DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::AssistantMessageCompleted {
                                message_id: "msg_assistant_latest".into(),
                                content: assistant.clone(),
                            },
                        ))
                        .await
                        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
                }
                ModelEvent::ToolCallRequested { tool_call_id, tool_id, .. } => {
                    self.store
                        .append(&DomainEvent::new(
                            request.workspace_id.clone(),
                            request.session_id.clone(),
                            AgentId::system(),
                            PrivacyClassification::FullTrace,
                            EventPayload::ModelToolCallRequested { tool_call_id, tool_id },
                        ))
                        .await
                        .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
                }
                ModelEvent::Failed { message } => {
                    return Err(agent_core::CoreError::InvalidState(message));
                }
            }
        }
        Ok(())
    }

    async fn decide_permission(&self, decision: PermissionDecision) -> agent_core::Result<()> {
        let _ = decision;
        Ok(())
    }

    async fn cancel_session(&self, workspace_id: WorkspaceId, session_id: SessionId) -> agent_core::Result<()> {
        self.store
            .append(&DomainEvent::new(
                workspace_id,
                session_id,
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::SessionCancelled { reason: "user requested cancellation".into() },
            ))
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
    }

    async fn get_session_projection(&self, session_id: SessionId) -> agent_core::Result<agent_core::projection::SessionProjection> {
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(agent_core::projection::SessionProjection::from_events(&events))
    }

    async fn get_trace(&self, session_id: SessionId) -> agent_core::Result<Vec<TraceEntry>> {
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(events.into_iter().map(|event| TraceEntry { event }).collect())
    }

    fn subscribe_session(&self, session_id: SessionId) -> futures::stream::BoxStream<'static, DomainEvent> {
        let _ = session_id;
        Box::pin(stream::empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
    use agent_models::FakeModelClient;
    use agent_store::SqliteEventStore;

    #[tokio::test]
    async fn send_message_records_user_and_assistant_events() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hello".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/workspace".into()).await.unwrap();
        let session_id = runtime.start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        }).await.unwrap();

        runtime.send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hi".into(),
        }).await.unwrap();

        let projection = runtime.get_session_projection(session_id).await.unwrap();
        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].content, "hi");
        assert_eq!(projection.messages[1].content, "hello");
    }
}
```

Update `crates/agent-runtime/src/lib.rs`:

```rust
pub mod facade_runtime;

pub use facade_runtime::LocalRuntime;
```

- [ ] **Step 6: Verify**

Run: `cargo test -p agent-runtime`

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/agent-models crates/agent-runtime
git commit -m "feat(runtime): run fake model session through facade"
```

---

### Task 6: TUI Single-Agent Shell

**Files:**
- Create: `crates/agent-tui/src/app.rs`
- Create: `crates/agent-tui/src/view.rs`
- Modify: `crates/agent-tui/src/main.rs`

- [ ] **Step 1: Write projection-to-view test**

Create `crates/agent-tui/src/view.rs`:

```rust
use agent_core::projection::{ProjectedMessage, ProjectedRole, SessionProjection};

pub fn render_lines(projection: &SessionProjection) -> Vec<String> {
    projection
        .messages
        .iter()
        .map(|message| match message.role {
            ProjectedRole::User => format!("You: {}", message.content),
            ProjectedRole::Assistant => format!("Agent: {}", message.content),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_chat_messages_from_projection() {
        let projection = SessionProjection {
            messages: vec![
                ProjectedMessage { role: ProjectedRole::User, content: "hi".into() },
                ProjectedMessage { role: ProjectedRole::Assistant, content: "hello".into() },
            ],
            task_titles: vec!["Session using fake".into()],
            token_stream: String::new(),
            cancelled: false,
        };

        assert_eq!(render_lines(&projection), vec!["You: hi", "Agent: hello"]);
    }
}
```

- [ ] **Step 2: Run test**

Run: `cargo test -p agent-tui renders_chat_messages_from_projection`

Expected: PASS after `view.rs` is wired into the binary module.

- [ ] **Step 3: Implement TUI app state and main**

Create `crates/agent-tui/src/app.rs`:

```rust
use agent_core::projection::SessionProjection;

#[derive(Debug, Default)]
pub struct TuiApp {
    pub projection: SessionProjection,
    pub input: String,
    pub status: String,
}

impl TuiApp {
    pub fn set_projection(&mut self, projection: SessionProjection) {
        self.projection = projection;
    }

    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = status.into();
    }
}
```

Update `crates/agent-tui/src/main.rs`:

```rust
mod app;
mod view;

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = SqliteEventStore::in_memory().await?;
    let model = FakeModelClient::new(vec!["hello from fake model".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime.open_workspace(std::env::current_dir()?.display().to_string()).await?;
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await?;
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
        })
        .await?;

    let projection = runtime.get_session_projection(session_id).await?;
    for line in view::render_lines(&projection) {
        println!("{line}");
    }

    Ok(())
}
```

- [ ] **Step 4: Verify CLI smoke path**

Run: `cargo run -p agent-tui`

Expected stdout contains:

```text
You: hello
Agent: hello from fake model
```

- [ ] **Step 5: Commit**

```bash
git add crates/agent-tui
git commit -m "feat(tui): show fake session through shared facade"
```

---

### Task 7: Permission Engine and Local Tools

**Files:**
- Create: `crates/agent-tools/src/permission.rs`
- Create: `crates/agent-tools/src/registry.rs`
- Create: `crates/agent-tools/src/filesystem.rs`
- Create: `crates/agent-tools/src/search.rs`
- Create: `crates/agent-tools/src/shell.rs`
- Create: `crates/agent-tools/src/patch.rs`
- Modify: `crates/agent-tools/src/lib.rs`

- [ ] **Step 1: Write permission mode tests**

Create `crates/agent-tools/src/permission.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readonly_allows_reads_and_blocks_shell_writes() {
        let engine = PermissionEngine::new(PermissionMode::ReadOnly);

        assert_eq!(engine.decide(&ToolRisk::read("fs.read")), PermissionOutcome::Allowed);
        assert_eq!(engine.decide(&ToolRisk::write("fs.write")), PermissionOutcome::Denied("read-only mode blocks writes".into()));
        assert_eq!(engine.decide(&ToolRisk::shell("shell.exec", false)), PermissionOutcome::Denied("read-only mode blocks shell execution".into()));
    }

    #[test]
    fn suggest_requires_approval_for_effectful_tools() {
        let engine = PermissionEngine::new(PermissionMode::Suggest);

        assert_eq!(engine.decide(&ToolRisk::write("patch.apply")), PermissionOutcome::RequiresApproval);
    }

    #[test]
    fn autonomous_still_requires_approval_for_destructive_shell() {
        let engine = PermissionEngine::new(PermissionMode::Autonomous);

        assert_eq!(engine.decide(&ToolRisk::shell("shell.exec", true)), PermissionOutcome::RequiresApproval);
    }
}
```

- [ ] **Step 2: Implement permission engine**

Replace `crates/agent-tools/src/permission.rs` with:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionMode {
    ReadOnly,
    Suggest,
    Agent,
    Autonomous,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionOutcome {
    Allowed,
    RequiresApproval,
    Denied(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolEffect {
    Read,
    Write,
    Shell { destructive: bool },
    Network,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolRisk {
    pub tool_id: String,
    pub effect: ToolEffect,
}

impl ToolRisk {
    pub fn read(tool_id: impl Into<String>) -> Self {
        Self { tool_id: tool_id.into(), effect: ToolEffect::Read }
    }

    pub fn write(tool_id: impl Into<String>) -> Self {
        Self { tool_id: tool_id.into(), effect: ToolEffect::Write }
    }

    pub fn shell(tool_id: impl Into<String>, destructive: bool) -> Self {
        Self { tool_id: tool_id.into(), effect: ToolEffect::Shell { destructive } }
    }
}

#[derive(Debug, Clone)]
pub struct PermissionEngine {
    mode: PermissionMode,
}

impl PermissionEngine {
    pub fn new(mode: PermissionMode) -> Self {
        Self { mode }
    }

    pub fn decide(&self, risk: &ToolRisk) -> PermissionOutcome {
        match (self.mode, &risk.effect) {
            (PermissionMode::ReadOnly, ToolEffect::Read) => PermissionOutcome::Allowed,
            (PermissionMode::ReadOnly, ToolEffect::Write) => PermissionOutcome::Denied("read-only mode blocks writes".into()),
            (PermissionMode::ReadOnly, ToolEffect::Shell { .. }) => PermissionOutcome::Denied("read-only mode blocks shell execution".into()),
            (PermissionMode::ReadOnly, ToolEffect::Network) => PermissionOutcome::Denied("read-only mode blocks network access".into()),
            (PermissionMode::Suggest, ToolEffect::Read) => PermissionOutcome::Allowed,
            (PermissionMode::Suggest, _) => PermissionOutcome::RequiresApproval,
            (PermissionMode::Agent, ToolEffect::Read) => PermissionOutcome::Allowed,
            (PermissionMode::Agent, ToolEffect::Write) => PermissionOutcome::Allowed,
            (PermissionMode::Agent, ToolEffect::Shell { destructive: false }) => PermissionOutcome::Allowed,
            (PermissionMode::Agent, _) => PermissionOutcome::RequiresApproval,
            (PermissionMode::Autonomous, ToolEffect::Shell { destructive: true }) => PermissionOutcome::RequiresApproval,
            (PermissionMode::Autonomous, ToolEffect::Network) => PermissionOutcome::RequiresApproval,
            (PermissionMode::Autonomous, _) => PermissionOutcome::Allowed,
        }
    }
}
```

- [ ] **Step 3: Implement registry envelope**

Create `crates/agent-tools/src/registry.rs`:

```rust
use crate::permission::{PermissionEngine, PermissionOutcome, ToolRisk};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub tool_id: String,
    pub description: String,
    pub required_capability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub tool_id: String,
    pub arguments: serde_json::Value,
    pub workspace_id: String,
    pub preview: String,
    pub timeout_ms: u64,
    pub output_limit_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolOutput {
    pub text: String,
    pub truncated: bool,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk;
    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput>;
}

pub fn require_permission(engine: &PermissionEngine, risk: &ToolRisk) -> crate::Result<()> {
    match engine.decide(risk) {
        PermissionOutcome::Allowed => Ok(()),
        PermissionOutcome::RequiresApproval => Err(crate::ToolError::PermissionRequired(risk.tool_id.clone())),
        PermissionOutcome::Denied(reason) => Err(crate::ToolError::PermissionDenied(reason)),
    }
}
```

- [ ] **Step 4: Implement initial filesystem read tool**

Create `crates/agent-tools/src/filesystem.rs`:

```rust
use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct FsReadTool {
    workspace_root: PathBuf,
}

impl FsReadTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }

    fn resolve_workspace_path(&self, relative_path: &str) -> crate::Result<PathBuf> {
        let candidate = self.workspace_root.join(relative_path);
        let root = self.workspace_root.canonicalize()?;
        let path = candidate.canonicalize()?;
        if path.starts_with(&root) {
            Ok(path)
        } else {
            Err(crate::ToolError::WorkspaceEscape(relative_path.into()))
        }
    }
}

#[async_trait]
impl Tool for FsReadTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "fs.read".into(),
            description: "Read a UTF-8 file within the workspace".into(),
            required_capability: "filesystem.read".into(),
        }
    }

    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk {
        let _ = invocation;
        ToolRisk::read("fs.read")
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let relative_path = invocation.arguments["path"].as_str().unwrap_or("");
        let path = self.resolve_workspace_path(relative_path)?;
        let mut text = tokio::fs::read_to_string(Path::new(&path)).await?;
        let truncated = text.len() > invocation.output_limit_bytes;
        if truncated {
            text.truncate(invocation.output_limit_bytes);
        }
        Ok(ToolOutput { text, truncated })
    }
}
```

Create placeholder modules that compile and document the later local tools:

```rust
// crates/agent-tools/src/search.rs
pub const SEARCH_TOOL_ID: &str = "search.ripgrep";
```

```rust
// crates/agent-tools/src/shell.rs
pub const SHELL_TOOL_ID: &str = "shell.exec";
```

```rust
// crates/agent-tools/src/patch.rs
pub const PATCH_TOOL_ID: &str = "patch.apply";
```

Update `crates/agent-tools/src/lib.rs`:

```rust
pub mod filesystem;
pub mod patch;
pub mod permission;
pub mod registry;
pub mod search;
pub mod shell;

pub use filesystem::FsReadTool;
pub use permission::{PermissionEngine, PermissionMode, PermissionOutcome, ToolEffect, ToolRisk};
pub use registry::{require_permission, Tool, ToolDefinition, ToolInvocation, ToolOutput};

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("permission required for {0}")]
    PermissionRequired(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("path escapes workspace: {0}")]
    WorkspaceEscape(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, ToolError>;
```

- [ ] **Step 5: Verify**

Run: `cargo test -p agent-tools`

Expected: PASS with permission mode tests.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-tools
git commit -m "feat(tools): add permission engine and filesystem read tool"
```

---

### Task 8: Model Profiles, OpenAI-Compatible, and Ollama Adapter Boundaries

**Files:**
- Create: `crates/agent-models/src/profile.rs`
- Create: `crates/agent-models/src/openai_compatible.rs`
- Create: `crates/agent-models/src/ollama.rs`
- Modify: `crates/agent-models/src/types.rs`
- Modify: `crates/agent-models/src/lib.rs`

- [ ] **Step 1: Write capability/profile tests**

Create `crates/agent-models/src/profile.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_exposes_capabilities_without_ui_types() {
        let profile = ModelProfile {
            alias: "fast".into(),
            provider: "openai_compatible".into(),
            model_id: "gpt-4.1-mini".into(),
            capabilities: ModelCapabilities {
                streaming: true,
                tool_calling: true,
                json_schema: true,
                vision: false,
                reasoning_controls: false,
                context_window: 128_000,
                output_limit: 16_384,
                local_model: false,
            },
        };

        assert_eq!(profile.alias, "fast");
        assert!(profile.capabilities.tool_calling);
        assert!(!profile.capabilities.local_model);
    }
}
```

- [ ] **Step 2: Implement profile and provider boundaries**

Replace `crates/agent-models/src/profile.rs` with:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelCapabilities {
    pub streaming: bool,
    pub tool_calling: bool,
    pub json_schema: bool,
    pub vision: bool,
    pub reasoning_controls: bool,
    pub context_window: u64,
    pub output_limit: u64,
    pub local_model: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelProfile {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub capabilities: ModelCapabilities,
}
```

Create `crates/agent-models/src/openai_compatible.rs`:

```rust
use crate::profile::ModelCapabilities;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAiCompatibleConfig {
    pub base_url: String,
    pub api_key_env: String,
    pub default_model: String,
    pub headers: Vec<(String, String)>,
    pub capability_overrides: Option<ModelCapabilities>,
}

impl OpenAiCompatibleConfig {
    pub fn default_capabilities(&self) -> ModelCapabilities {
        self.capability_overrides.clone().unwrap_or(ModelCapabilities {
            streaming: true,
            tool_calling: true,
            json_schema: true,
            vision: false,
            reasoning_controls: false,
            context_window: 128_000,
            output_limit: 16_384,
            local_model: false,
        })
    }
}
```

Create `crates/agent-models/src/ollama.rs`:

```rust
use crate::profile::ModelCapabilities;

pub fn ollama_default_capabilities(context_window: u64) -> ModelCapabilities {
    ModelCapabilities {
        streaming: true,
        tool_calling: false,
        json_schema: false,
        vision: false,
        reasoning_controls: false,
        context_window,
        output_limit: 4096,
        local_model: true,
    }
}
```

Update `crates/agent-models/src/lib.rs`:

```rust
pub mod fake;
pub mod ollama;
pub mod openai_compatible;
pub mod profile;
pub mod types;

pub use fake::FakeModelClient;
pub use profile::{ModelCapabilities, ModelProfile};
pub use types::{ModelClient, ModelEvent, ModelMessage, ModelRequest, ModelUsage};

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model request failed: {0}")]
    Request(String),
}

pub type Result<T> = std::result::Result<T, ModelError>;
```

- [ ] **Step 3: Verify**

Run: `cargo test -p agent-models`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-models
git commit -m "feat(models): define profiles and provider adapter boundaries"
```

---

### Task 9: Tauri/Vue GUI Shell

**Files:**
- Create: `apps/agent-gui/package.json`
- Create: `apps/agent-gui/index.html`
- Create: `apps/agent-gui/vite.config.ts`
- Create: `apps/agent-gui/tsconfig.json`
- Create: `apps/agent-gui/src/main.ts`
- Create: `apps/agent-gui/src/App.vue`
- Create: `apps/agent-gui/src/components/TraceTimeline.vue`
- Create: `apps/agent-gui/src/components/PermissionCenter.vue`
- Create: `apps/agent-gui/src-tauri/Cargo.toml`
- Create: `apps/agent-gui/src-tauri/tauri.conf.json`
- Create: `apps/agent-gui/src-tauri/src/lib.rs`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add GUI workspace member**

Update root `Cargo.toml` members:

```toml
members = [
  "crates/agent-core",
  "crates/agent-runtime",
  "crates/agent-models",
  "crates/agent-tools",
  "crates/agent-memory",
  "crates/agent-store",
  "crates/agent-tui",
  "apps/agent-gui/src-tauri",
]
```

- [ ] **Step 2: Write Vue projection fixture test**

Create `apps/agent-gui/package.json`:

```json
{
  "name": "agent-gui",
  "private": true,
  "version": "0.1.0",
  "type": "module",
  "scripts": {
    "dev": "vite",
    "test": "vitest run",
    "build": "vite build"
  },
  "dependencies": {
    "@tauri-apps/api": "^2.0.0",
    "vue": "^3.5.0"
  },
  "devDependencies": {
    "@vitejs/plugin-vue": "^5.2.0",
    "typescript": "^5.6.0",
    "vite": "^6.0.0",
    "vitest": "^2.1.0",
    "vue-tsc": "^2.1.0"
  }
}
```

Create `apps/agent-gui/src/components/TraceTimeline.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { traceLabels } from "./TraceTimeline";

describe("traceLabels", () => {
  it("renders event types in order", () => {
    expect(traceLabels([{ event_type: "UserMessageAdded" }, { event_type: "AssistantMessageCompleted" }])).toEqual([
      "UserMessageAdded",
      "AssistantMessageCompleted",
    ]);
  });
});
```

- [ ] **Step 3: Implement Vue shell and trace utility**

Create `apps/agent-gui/vite.config.ts`:

```ts
import vue from "@vitejs/plugin-vue";
import { defineConfig } from "vite";

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
});
```

Create `apps/agent-gui/tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2022",
    "module": "ESNext",
    "moduleResolution": "Bundler",
    "strict": true,
    "jsx": "preserve",
    "sourceMap": true,
    "resolveJsonModule": true,
    "isolatedModules": true,
    "lib": ["ES2022", "DOM", "DOM.Iterable"],
    "types": ["vitest/globals"]
  },
  "include": ["src/**/*.ts", "src/**/*.vue"]
}
```

Create `apps/agent-gui/index.html`:

```html
<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <title>Kairox Agent Workbench</title>
  </head>
  <body>
    <div id="app"></div>
    <script type="module" src="/src/main.ts"></script>
  </body>
</html>
```

Create `apps/agent-gui/src/main.ts`:

```ts
import { createApp } from "vue";
import App from "./App.vue";

createApp(App).mount("#app");
```

Create `apps/agent-gui/src/components/TraceTimeline.ts`:

```ts
export type TraceEvent = {
  event_type: string;
};

export function traceLabels(events: TraceEvent[]): string[] {
  return events.map((event) => event.event_type);
}
```

Create `apps/agent-gui/src/components/TraceTimeline.vue`:

```vue
<script setup lang="ts">
import { computed } from "vue";
import { traceLabels, type TraceEvent } from "./TraceTimeline";

const props = defineProps<{ events: TraceEvent[] }>();
const labels = computed(() => traceLabels(props.events));
</script>

<template>
  <section class="trace">
    <h2>Trace</h2>
    <ol>
      <li v-for="label in labels" :key="label">{{ label }}</li>
    </ol>
  </section>
</template>
```

Create `apps/agent-gui/src/components/PermissionCenter.vue`:

```vue
<template>
  <section class="permission-center">
    <h2>Permissions</h2>
    <p>No pending permission requests.</p>
  </section>
</template>
```

Create `apps/agent-gui/src/App.vue`:

```vue
<script setup lang="ts">
import PermissionCenter from "./components/PermissionCenter.vue";
import TraceTimeline from "./components/TraceTimeline.vue";

const events = [{ event_type: "WorkspaceOpened" }, { event_type: "UserMessageAdded" }];
</script>

<template>
  <main class="workbench">
    <aside class="sidebar">
      <h1>Kairox</h1>
      <p>Local workbench</p>
    </aside>
    <section class="session">
      <h2>Session</h2>
      <p>Shared core session projection will render here.</p>
    </section>
    <TraceTimeline :events="events" />
    <PermissionCenter />
  </main>
</template>

<style scoped>
.workbench {
  display: grid;
  grid-template-columns: 220px 1fr 320px;
  min-height: 100vh;
  font-family: system-ui, sans-serif;
}
.sidebar,
.session,
.trace,
.permission-center {
  padding: 16px;
  border-right: 1px solid #d7d7d7;
}
</style>
```

- [ ] **Step 4: Add Tauri command bridge**

Create `apps/agent-gui/src-tauri/Cargo.toml`:

```toml
[package]
name = "agent-gui-tauri"
version.workspace = true
edition.workspace = true
license.workspace = true

[lib]
name = "agent_gui_tauri"
crate-type = ["staticlib", "cdylib", "rlib"]

[dependencies]
agent-core = { path = "../../../crates/agent-core" }
serde.workspace = true
serde_json.workspace = true
tauri = { version = "2", features = [] }
```

Create `apps/agent-gui/src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Kairox",
  "version": "0.1.0",
  "identifier": "dev.kairox.agent",
  "build": {
    "beforeDevCommand": "npm run dev",
    "beforeBuildCommand": "npm run build",
    "devUrl": "http://localhost:1420",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Kairox",
        "width": 1200,
        "height": 800
      }
    ]
  }
}
```

Create `apps/agent-gui/src-tauri/src/lib.rs`:

```rust
#[tauri::command]
pub fn list_model_profiles() -> Vec<String> {
    vec!["fake".into(), "fast".into(), "local-code".into(), "reviewer".into()]
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![list_model_profiles])
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_default_profiles() {
        assert!(list_model_profiles().contains(&"fake".to_string()));
    }
}
```

- [ ] **Step 5: Verify GUI units**

Run: `cd apps/agent-gui && npm install`

Expected: dependencies install successfully.

Run: `cd apps/agent-gui && npm test`

Expected: PASS for `TraceTimeline.test.ts`.

Run: `cargo test -p agent-gui-tauri`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml apps/agent-gui
git commit -m "feat(gui): scaffold tauri vue workbench shell"
```

---

### Task 10: Memory and Context Assembly

**Files:**
- Create: `crates/agent-memory/src/memory.rs`
- Create: `crates/agent-memory/src/context.rs`
- Modify: `crates/agent-memory/src/lib.rs`

- [ ] **Step 1: Write memory proposal and context tests**

Create `crates/agent-memory/src/context.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryEntry, MemoryScope};

    #[test]
    fn assembles_request_history_and_workspace_memory_within_budget() {
        let assembler = ContextAssembler::new(100);
        let bundle = assembler.assemble(ContextRequest {
            user_request: "fix tests".into(),
            session_history: vec!["previous answer".into()],
            selected_files: vec!["Cargo.toml".into()],
            tool_results: vec!["cargo test failed".into()],
            memories: vec![MemoryEntry {
                id: "mem1".into(),
                scope: MemoryScope::Workspace,
                content: "Use cargo test --workspace".into(),
                accepted: true,
            }],
            active_task: Some("repair failing test".into()),
        });

        assert!(bundle.messages.join("\n").contains("fix tests"));
        assert!(bundle.messages.join("\n").contains("Use cargo test --workspace"));
        assert!(bundle.token_estimate <= 100);
    }
}
```

- [ ] **Step 2: Implement memory and context types**

Create `crates/agent-memory/src/memory.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryScope {
    User,
    Workspace,
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEntry {
    pub id: String,
    pub scope: MemoryScope,
    pub content: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryDecision {
    Accept,
    Reject(String),
}

pub fn durable_memory_requires_confirmation(scope: &MemoryScope) -> bool {
    matches!(scope, MemoryScope::User | MemoryScope::Workspace)
}
```

Replace `crates/agent-memory/src/context.rs` with:

```rust
use crate::memory::MemoryEntry;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextRequest {
    pub user_request: String,
    pub session_history: Vec<String>,
    pub selected_files: Vec<String>,
    pub tool_results: Vec<String>,
    pub memories: Vec<MemoryEntry>,
    pub active_task: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextBundle {
    pub messages: Vec<String>,
    pub token_estimate: usize,
    pub sources: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ContextAssembler {
    max_tokens: usize,
}

impl ContextAssembler {
    pub fn new(max_tokens: usize) -> Self {
        Self { max_tokens }
    }

    pub fn assemble(&self, request: ContextRequest) -> ContextBundle {
        let mut messages = Vec::new();
        messages.push(format!("User request: {}", request.user_request));
        if let Some(active_task) = request.active_task {
            messages.push(format!("Active task: {active_task}"));
        }
        messages.extend(request.session_history.into_iter().map(|item| format!("History: {item}")));
        messages.extend(request.selected_files.into_iter().map(|item| format!("Selected file: {item}")));
        messages.extend(request.tool_results.into_iter().map(|item| format!("Tool result: {item}")));
        messages.extend(
            request
                .memories
                .into_iter()
                .filter(|memory| memory.accepted)
                .map(|memory| format!("Memory: {}", memory.content)),
        );

        let mut token_estimate = estimate_tokens(&messages.join("\n"));
        while token_estimate > self.max_tokens && messages.len() > 1 {
            messages.remove(1);
            token_estimate = estimate_tokens(&messages.join("\n"));
        }

        ContextBundle {
            sources: messages.iter().map(|message| message.split(':').next().unwrap_or("context").to_string()).collect(),
            messages,
            token_estimate,
        }
    }
}

fn estimate_tokens(text: &str) -> usize {
    text.split_whitespace().count().max(1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MemoryEntry, MemoryScope};

    #[test]
    fn assembles_request_history_and_workspace_memory_within_budget() {
        let assembler = ContextAssembler::new(100);
        let bundle = assembler.assemble(ContextRequest {
            user_request: "fix tests".into(),
            session_history: vec!["previous answer".into()],
            selected_files: vec!["Cargo.toml".into()],
            tool_results: vec!["cargo test failed".into()],
            memories: vec![MemoryEntry {
                id: "mem1".into(),
                scope: MemoryScope::Workspace,
                content: "Use cargo test --workspace".into(),
                accepted: true,
            }],
            active_task: Some("repair failing test".into()),
        });

        assert!(bundle.messages.join("\n").contains("fix tests"));
        assert!(bundle.messages.join("\n").contains("Use cargo test --workspace"));
        assert!(bundle.token_estimate <= 100);
    }
}
```

Update `crates/agent-memory/src/lib.rs`:

```rust
pub mod context;
pub mod memory;

pub use context::{ContextAssembler, ContextBundle, ContextRequest};
pub use memory::{durable_memory_requires_confirmation, MemoryDecision, MemoryEntry, MemoryScope};
```

- [ ] **Step 3: Verify**

Run: `cargo test -p agent-memory`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-memory
git commit -m "feat(memory): assemble local context from accepted memory"
```

---

### Task 11: MCP and Manifest Discovery

**Files:**
- Create: `crates/agent-tools/src/mcp.rs`
- Create: `crates/agent-core/src/manifest.rs`
- Create: `fixtures/extensions/sample-skill/skill.toml`
- Create: `fixtures/extensions/sample-plugin/plugin.toml`
- Modify: `crates/agent-tools/src/lib.rs`
- Modify: `crates/agent-core/src/lib.rs`

- [ ] **Step 1: Write manifest parsing tests**

Create `crates/agent-core/src/manifest.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skill_manifest() {
        let manifest: ExtensionManifest = toml::from_str(r#"
id = "skill.code-review"
name = "Code Review"
version = "0.1.0"
description = "Review code changes"
extension_type = "skill"
triggers = ["review"]
prompt_templates = ["Check correctness and tests"]
required_tools = ["git.diff"]
required_permissions = ["filesystem.read"]
core_version = ">=0.1.0"
"#).unwrap();

        assert_eq!(manifest.id, "skill.code-review");
        assert_eq!(manifest.extension_type, ExtensionType::Skill);
        assert_eq!(manifest.required_tools, vec!["git.diff"]);
    }
}
```

- [ ] **Step 2: Implement manifest schema**

Replace `crates/agent-core/src/manifest.rs` with:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionType {
    Skill,
    Plugin,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub extension_type: ExtensionType,
    pub triggers: Vec<String>,
    pub prompt_templates: Vec<String>,
    pub required_tools: Vec<String>,
    pub required_permissions: Vec<String>,
    pub core_version: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_skill_manifest() {
        let manifest: ExtensionManifest = toml::from_str(r#"
id = "skill.code-review"
name = "Code Review"
version = "0.1.0"
description = "Review code changes"
extension_type = "skill"
triggers = ["review"]
prompt_templates = ["Check correctness and tests"]
required_tools = ["git.diff"]
required_permissions = ["filesystem.read"]
core_version = ">=0.1.0"
"#).unwrap();

        assert_eq!(manifest.id, "skill.code-review");
        assert_eq!(manifest.extension_type, ExtensionType::Skill);
        assert_eq!(manifest.required_tools, vec!["git.diff"]);
    }
}
```

Update `crates/agent-core/src/lib.rs` to add:

```rust
pub mod manifest;
pub use manifest::{ExtensionManifest, ExtensionType};
```

Add `toml.workspace = true` to `crates/agent-core/Cargo.toml`.

- [ ] **Step 3: Implement MCP boundary**

Create `crates/agent-tools/src/mcp.rs`:

```rust
use crate::registry::ToolDefinition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerConfig {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpTool {
    pub server_id: String,
    pub definition: ToolDefinition,
}

pub fn map_mcp_tool(server_id: impl Into<String>, name: impl Into<String>, description: impl Into<String>) -> McpTool {
    let name = name.into();
    McpTool {
        server_id: server_id.into(),
        definition: ToolDefinition {
            tool_id: format!("mcp.{name}"),
            description: description.into(),
            required_capability: "mcp.invoke".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_tool_maps_to_shared_tool_definition() {
        let tool = map_mcp_tool("local", "read_doc", "Read a doc");
        assert_eq!(tool.server_id, "local");
        assert_eq!(tool.definition.tool_id, "mcp.read_doc");
        assert_eq!(tool.definition.required_capability, "mcp.invoke");
    }
}
```

Update `crates/agent-tools/src/lib.rs`:

```rust
pub mod mcp;
pub use mcp::{map_mcp_tool, McpServerConfig, McpTool};
```

- [ ] **Step 4: Add fixture manifests**

Create `fixtures/extensions/sample-skill/skill.toml`:

```toml
id = "skill.sample-review"
name = "Sample Review"
version = "0.1.0"
description = "Reviews simple local changes"
extension_type = "skill"
triggers = ["review", "diff"]
prompt_templates = ["Review correctness, tests, and permission risk."]
required_tools = ["git.diff"]
required_permissions = ["filesystem.read"]
core_version = ">=0.1.0"
```

Create `fixtures/extensions/sample-plugin/plugin.toml`:

```toml
id = "plugin.sample-tools"
name = "Sample Tools"
version = "0.1.0"
description = "Declares sample local tool capabilities"
extension_type = "plugin"
triggers = ["sample"]
prompt_templates = ["Use sample tools when explicitly requested."]
required_tools = ["mcp.read_doc"]
required_permissions = ["mcp.invoke"]
core_version = ">=0.1.0"
```

- [ ] **Step 5: Verify**

Run: `cargo test -p agent-core parses_skill_manifest`

Expected: PASS.

Run: `cargo test -p agent-tools mcp_tool_maps_to_shared_tool_definition`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent-core crates/agent-tools fixtures/extensions
git commit -m "feat(extensions): parse manifests and map mcp tools"
```

---

### Task 12: Multi-Agent Task Graph

**Files:**
- Create: `crates/agent-runtime/src/task_graph.rs`
- Create: `crates/agent-runtime/src/agents.rs`
- Modify: `crates/agent-runtime/src/lib.rs`

- [ ] **Step 1: Write scheduler tests**

Create `crates/agent-runtime/src/task_graph.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_ready_tasks_and_blocks_dependents() {
        let mut graph = TaskGraph::default();
        let plan = graph.add_task("plan", AgentRole::Planner, vec![]);
        let work = graph.add_task("work", AgentRole::Worker, vec![plan.clone()]);

        assert_eq!(graph.ready_tasks(), vec![plan.clone()]);
        graph.mark_completed(&plan).unwrap();
        assert_eq!(graph.ready_tasks(), vec![work]);
    }
}
```

- [ ] **Step 2: Implement task graph**

Replace `crates/agent-runtime/src/task_graph.rs` with:

```rust
use agent_core::TaskId;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentRole {
    Planner,
    Worker,
    Reviewer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Running,
    Blocked,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentTask {
    pub id: TaskId,
    pub title: String,
    pub role: AgentRole,
    pub state: TaskState,
    pub dependencies: Vec<TaskId>,
}

#[derive(Debug, Default)]
pub struct TaskGraph {
    tasks: BTreeMap<String, AgentTask>,
}

impl TaskGraph {
    pub fn add_task(&mut self, title: impl Into<String>, role: AgentRole, dependencies: Vec<TaskId>) -> TaskId {
        let id = TaskId::new();
        let task = AgentTask {
            id: id.clone(),
            title: title.into(),
            role,
            state: TaskState::Pending,
            dependencies,
        };
        self.tasks.insert(id.to_string(), task);
        id
    }

    pub fn ready_tasks(&self) -> Vec<TaskId> {
        let completed: BTreeSet<String> = self
            .tasks
            .values()
            .filter(|task| task.state == TaskState::Completed)
            .map(|task| task.id.to_string())
            .collect();
        self.tasks
            .values()
            .filter(|task| {
                task.state == TaskState::Pending
                    && task.dependencies.iter().all(|dependency| completed.contains(&dependency.to_string()))
            })
            .map(|task| task.id.clone())
            .collect()
    }

    pub fn mark_completed(&mut self, id: &TaskId) -> crate::Result<()> {
        let task = self
            .tasks
            .get_mut(&id.to_string())
            .ok_or_else(|| crate::RuntimeError::UnknownTask(id.to_string()))?;
        task.state = TaskState::Completed;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedules_ready_tasks_and_blocks_dependents() {
        let mut graph = TaskGraph::default();
        let plan = graph.add_task("plan", AgentRole::Planner, vec![]);
        let work = graph.add_task("work", AgentRole::Worker, vec![plan.clone()]);

        assert_eq!(graph.ready_tasks(), vec![plan.clone()]);
        graph.mark_completed(&plan).unwrap();
        assert_eq!(graph.ready_tasks(), vec![work]);
    }
}
```

Create `crates/agent-runtime/src/agents.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannerAgent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerAgent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerAgent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerFinding {
    pub severity: String,
    pub message: String,
}

impl ReviewerAgent {
    pub fn review_diff(diff: &str) -> Vec<ReviewerFinding> {
        let mut findings = Vec::new();
        if diff.contains("rm -rf") {
            findings.push(ReviewerFinding {
                severity: "high".into(),
                message: "destructive shell command requires explicit approval".into(),
            });
        }
        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reviewer_flags_destructive_commands() {
        let findings = ReviewerAgent::review_diff("+ rm -rf target");
        assert_eq!(findings[0].severity, "high");
    }
}
```

Update `crates/agent-runtime/src/lib.rs`:

```rust
pub mod agents;
pub mod facade_runtime;
pub mod task_graph;

pub use agents::{PlannerAgent, ReviewerAgent, ReviewerFinding, WorkerAgent};
pub use facade_runtime::LocalRuntime;
pub use task_graph::{AgentRole, AgentTask, TaskGraph, TaskState};

#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("unknown task: {0}")]
    UnknownTask(String),
}

pub type Result<T> = std::result::Result<T, RuntimeError>;
```

- [ ] **Step 3: Verify**

Run: `cargo test -p agent-runtime schedules_ready_tasks_and_blocks_dependents reviewer_flags_destructive_commands`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-runtime
git commit -m "feat(runtime): add multi-agent task graph"
```

---

### Task 13: Optional Account Boundary

**Files:**
- Create: `crates/agent-core/src/account.rs`
- Modify: `crates/agent-core/src/lib.rs`

- [ ] **Step 1: Write no-account service test**

Create `crates/agent-core/src/account.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_no_account_preserves_full_local_use() {
        let service = LocalNoAccountService;
        let state = service.current_account().await.unwrap();

        assert_eq!(state.login_required, false);
        assert_eq!(state.settings_sync_enabled, false);
        assert_eq!(state.subscription_plan, None);
    }
}
```

- [ ] **Step 2: Implement account trait**

Replace `crates/agent-core/src/account.rs` with:

```rust
use async_trait::async_trait;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountState {
    pub login_required: bool,
    pub settings_sync_enabled: bool,
    pub subscription_plan: Option<String>,
}

#[async_trait]
pub trait AccountService: Send + Sync {
    async fn current_account(&self) -> crate::Result<AccountState>;
}

#[derive(Debug, Clone)]
pub struct LocalNoAccountService;

#[async_trait]
impl AccountService for LocalNoAccountService {
    async fn current_account(&self) -> crate::Result<AccountState> {
        Ok(AccountState {
            login_required: false,
            settings_sync_enabled: false,
            subscription_plan: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_no_account_preserves_full_local_use() {
        let service = LocalNoAccountService;
        let state = service.current_account().await.unwrap();

        assert_eq!(state.login_required, false);
        assert_eq!(state.settings_sync_enabled, false);
        assert_eq!(state.subscription_plan, None);
    }
}
```

Update `crates/agent-core/src/lib.rs`:

```rust
pub mod account;
pub use account::{AccountService, AccountState, LocalNoAccountService};
```

- [ ] **Step 3: Verify**

Run: `cargo test -p agent-core local_no_account_preserves_full_local_use`

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-core
git commit -m "feat(core): add optional account boundary"
```

---

### Task 14: End-to-End Fake Session and Trace Replay

**Files:**
- Create: `crates/agent-runtime/tests/fake_session.rs`
- Create: `docs/dev/local-development.md`

- [ ] **Step 1: Add e2e test package entry**

No additional crate is required. Rust integration tests can live under root `tests/e2e` only if the root is a package. Because this repository root is a virtual workspace, create `crates/agent-runtime/tests/fake_session.rs` instead.

Use file:
- Create: `crates/agent-runtime/tests/fake_session.rs`

- [ ] **Step 2: Write e2e test**

Create `crates/agent-runtime/tests/fake_session.rs`:

```rust
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

#[tokio::test]
async fn fake_model_completes_full_session_and_trace_replays() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["done".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime.open_workspace("/tmp/kairox-e2e".into()).await.unwrap();
    let session_id = runtime.start_session(StartSessionRequest {
        workspace_id: workspace.workspace_id.clone(),
        model_profile: "fake".into(),
    }).await.unwrap();

    runtime.send_message(SendMessageRequest {
        workspace_id: workspace.workspace_id,
        session_id: session_id.clone(),
        content: "complete this".into(),
    }).await.unwrap();

    let trace = runtime.get_trace(session_id.clone()).await.unwrap();
    let event_types: Vec<_> = trace.iter().map(|entry| entry.event.event_type).collect();
    assert!(event_types.contains(&"UserMessageAdded"));
    assert!(event_types.contains(&"ModelTokenDelta"));
    assert!(event_types.contains(&"AssistantMessageCompleted"));

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(projection.messages.last().unwrap().content, "done");
}
```

- [ ] **Step 3: Create developer commands doc**

Create `docs/dev/local-development.md`:

````markdown
# Local Development

## Rust

Run all Rust tests:

```bash
cargo test --workspace
```

Run the TUI fake session:

```bash
cargo run -p agent-tui
```

## GUI

Install frontend dependencies:

```bash
cd apps/agent-gui && npm install
```

Run Vue unit tests:

```bash
cd apps/agent-gui && npm test
```

Run the Vite development server:

```bash
cd apps/agent-gui && npm run dev
```

## Privacy Defaults

The initial runtime stores event envelopes and full fake-session content in SQLite during tests. Production configuration must default to `minimal_trace` when a real model or shell tool is configured.
````

- [ ] **Step 4: Verify full workspace**

Run: `cargo test --workspace`

Expected: PASS.

Run: `cd apps/agent-gui && npm test`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-runtime/tests docs/dev
git commit -m "test: cover fake session trace replay"
```

---

## Acceptance Mapping

- TUI and GUI can run the same agent session through the same Rust core: Tasks 3, 5, 6, 9, 14.
- Model adapters can be added without changing UI code: Task 8.
- Tool calls always pass through the permission engine: Task 7, then integrate into runtime before real write/shell tools are enabled.
- Users can use the product without login: Task 13.
- Project memory improves context without requiring embeddings: Task 10.
- MCP tools share the same invocation and audit path as built-in tools: Task 11, then runtime integration in M4 follow-up.
- Multi-agent orchestration is useful without requiring a visual workflow canvas: Task 12.
- Trace replay can reconstruct meaningful session state for debugging and audit: Tasks 2, 4, 14.

## Follow-Up Plans After This Baseline

Create separate implementation plans for these larger post-baseline slices:

- `2026-04-29-tool-execution-hardening.md`: complete search, git, shell, patch application, workspace policy, command previews, and audit event integration.
- `2026-04-29-real-model-adapters.md`: HTTP clients for OpenAI-compatible and Ollama, credential validation, health checks, streaming protocol tests, and redaction.
- `2026-04-29-tauri-facade-integration.md`: long-lived runtime state in Tauri, command serialization, event subscriptions, and GUI projection fixtures.
- `2026-04-29-mcp-runtime-integration.md`: local server lifecycle, discovery, invocation, timeout, output limits, and permission prompts.
- `2026-04-29-agent-orchestration-runtime.md`: planner/worker/reviewer execution loop, parallel scheduling policy, checkpoints, and reviewer trace events.

## Self-Review

- Spec coverage: Every Phase 1 area is represented by at least one task. The plan intentionally stops real shell/network execution at safe boundaries and calls out follow-up plans for hardening.
- Placeholder scan: The plan contains no unresolved markers, no open-ended error-handling instructions, and no unnamed tests.
- Type consistency: `WorkspaceId`, `SessionId`, `TaskId`, `AgentId`, `DomainEvent`, `EventPayload`, `AppFacade`, `ModelClient`, `ToolRisk`, and `TaskGraph` names are consistent across tasks.
- Testing: Each subsystem has a failing-test-first step and a verification command. The full baseline closes with `cargo test --workspace` and GUI unit tests.
