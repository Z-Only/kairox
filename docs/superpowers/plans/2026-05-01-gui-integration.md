# GUI Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Bridge the Rust AppFacade to a Vue 3 frontend via Tauri 2 commands and events, delivering a Progressive MVP with live streaming chat, session management, and model profile switching.

**Architecture:** A `GuiState` struct wraps `LocalRuntime` and session tracking in the Tauri backend. Tauri commands invoke `AppFacade` methods. A background tokio task forwards `DomainEvent`s from `subscribe_session()` to the frontend via `app.emit()`. Vue components use reactive stores and a Tauri event listener composable.

**Tech Stack:** Rust, Tauri 2, Vue 3, TypeScript, `@tauri-apps/api`, `agent-core`, `agent-runtime`, `agent-store`, `agent-models`, `agent-tools`, vitest

---

## File Structure

### Rust (Tauri Backend)

| File                                              | Status | Responsibility                                              |
| ------------------------------------------------- | ------ | ----------------------------------------------------------- |
| `crates/agent-core/src/ids.rs`                    | Modify | Add from_string() and From<String> to ID types              |
| `apps/agent-gui/src-tauri/Cargo.toml`             | Modify | Add workspace crate dependencies                            |
| `apps/agent-gui/src-tauri/src/lib.rs`             | Modify | Replace with setup logic, state init, command registration  |
| `apps/agent-gui/src-tauri/src/app_state.rs`       | Create | `GuiState` struct with runtime, sessions, forwarder handles |
| `apps/agent-gui/src-tauri/src/commands.rs`        | Create | All Tauri command handlers                                  |
| `apps/agent-gui/src-tauri/src/event_forwarder.rs` | Create | DomainEvent → Tauri event bridge with session lifecycle     |

### Vue (Frontend)

| File                                                | Status | Responsibility                                            |
| --------------------------------------------------- | ------ | --------------------------------------------------------- |
| `apps/agent-gui/src/types/index.ts`                 | Create | TypeScript mirrors of Rust DomainEvent, SessionProjection |
| `apps/agent-gui/src/stores/session.ts`              | Create | Reactive session state and `applyEvent` projector         |
| `apps/agent-gui/src/composables/useTauriEvents.ts`  | Create | Tauri event listener lifecycle management                 |
| `apps/agent-gui/src/components/ChatPanel.vue`       | Create | Message list, streaming cursor, input area                |
| `apps/agent-gui/src/components/SessionsSidebar.vue` | Create | Session list, new session, profile selection              |
| `apps/agent-gui/src/components/StatusBar.vue`       | Create | Profile name, session count, connection status            |
| `apps/agent-gui/src/App.vue`                        | Modify | Updated layout with real data binding                     |
| `apps/agent-gui/src/main.ts`                        | Modify | Updated entry point                                       |

### Replaced/Removed

| File                                                  | Action | Reason                         |
| ----------------------------------------------------- | ------ | ------------------------------ |
| `apps/agent-gui/src/components/TraceTimeline.vue`     | Modify | Show "Coming soon" placeholder |
| `apps/agent-gui/src/components/TraceTimeline.ts`      | Modify | Remove unused types            |
| `apps/agent-gui/src/components/TraceTimeline.test.ts` | Modify | Update for placeholder         |
| `apps/agent-gui/src/components/PermissionCenter.vue`  | Modify | Show "Coming soon" placeholder |

---

## Task 0: Add from_string() Constructor to ID Types

The Tauri commands receive session IDs as `String` from the frontend, but the `SessionId` type is an opaque newtype without a `From<String>` impl. We need a safe constructor so commands can convert `String` → `SessionId`.

**Files:**

- Modify: `crates/agent-core/src/ids.rs`

- [ ] **Step 1: Add `from_string()` method to the `prefixed_id` macro**

In `crates/agent-core/src/ids.rs`, add a `from_string` method and a `From<String>` impl to the macro. The `from_string` method allows reconstructing an ID from a previously serialized string (e.g., received over Tauri IPC). Update the macro to:

```rust
macro_rules! prefixed_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, Uuid::new_v4().simple()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Reconstruct an ID from a previously serialized string.
            /// This should only be used when receiving IDs from external sources
            /// (e.g., Tauri frontend, API). Prefer `new()` for creating fresh IDs.
            pub fn from_string(s: String) -> Self {
                Self(s)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}
```

- [ ] **Step 2: Run workspace tests**

Run: `cargo test -p agent-core`
Expected: all existing tests pass (the new methods are additions, not breaking changes)

- [ ] **Step 3: Run full workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/agent-core/src/ids.rs
git commit -m "feat(core): add from_string() constructor and From<String> impl to ID types"
```

---

## Task 1: Update Tauri Cargo.toml with Runtime Dependencies

**Files:**

- Modify: `apps/agent-gui/src-tauri/Cargo.toml`

- [ ] **Step 1: Add workspace crate dependencies**

Update `apps/agent-gui/src-tauri/Cargo.toml` to:

```toml
[package]
name = "agent-gui-tauri"
build = "build.rs"
version.workspace = true
edition.workspace = true
license.workspace = true

[lib]
name = "agent_gui_tauri"
crate-type = ["staticlib", "cdylib", "rlib"]

[dependencies]
agent-core = { path = "../../../crates/agent-core" }
agent-models = { path = "../../../crates/agent-models" }
agent-runtime = { path = "../../../crates/agent-runtime" }
agent-store = { path = "../../../crates/agent-store" }
agent-tools = { path = "../../../crates/agent-tools" }
futures.workspace = true
serde.workspace = true
serde_json.workspace = true
tauri = { version = "2", features = [] }
tokio = { workspace = true, features = ["macros", "rt-multi-thread", "sync", "time", "process", "fs"] }

[build-dependencies]
tauri-build = { version = "2", features = [] }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p agent-gui-tauri`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src-tauri/Cargo.toml Cargo.lock
git commit -m "feat(gui): add runtime dependencies to Tauri backend"
```

---

## Task 2: Create GuiState and Event Forwarder

**Files:**

- Create: `apps/agent-gui/src-tauri/src/app_state.rs`
- Create: `apps/agent-gui/src-tauri/src/event_forwarder.rs`

- [ ] **Step 1: Create app_state.rs**

Create `apps/agent-gui/src-tauri/src/app_state.rs`:

```rust
use agent_core::{SessionId, WorkspaceId};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub struct WorkspaceSession {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub profile: String,
}

pub struct GuiState {
    pub runtime: Arc<LocalRuntime<SqliteEventStore, FakeModelClient>>,
    pub workspace_id: Mutex<Option<WorkspaceId>>,
    pub sessions: Mutex<HashMap<String, WorkspaceSession>>,
    pub current_session_id: Mutex<Option<SessionId>>,
    pub forwarder_handle: Mutex<Option<JoinHandle<()>>>,
}

impl GuiState {
    pub fn new(runtime: LocalRuntime<SqliteEventStore, FakeModelClient>) -> Self {
        Self {
            runtime: Arc::new(runtime),
            workspace_id: Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
            current_session_id: Mutex::new(None),
            forwarder_handle: Mutex::new(None),
        }
    }
}
```

- [ ] **Step 2: Create event_forwarder.rs**

Create `apps/agent-gui/src-tauri/src/event_forwarder.rs`:

```rust
use agent_core::DomainEvent;
use futures::StreamExt;
use tauri::AppHandle;
use tauri::Emitter;

/// Spawn a background task that forwards DomainEvents from the runtime
/// subscription to the Vue frontend via Tauri events.
/// Returns the JoinHandle so the caller can abort it on session switch.
pub fn spawn_event_forwarder(
    runtime: &agent_runtime::LocalRuntime<
        agent_store::SqliteEventStore,
        agent_models::FakeModelClient,
    >,
    session_id: agent_core::SessionId,
    app_handle: AppHandle,
) -> tokio::task::JoinHandle<()> {
    let mut stream = runtime.subscribe_session(session_id);

    tokio::spawn(async move {
        while let Some(event) = stream.next().await {
            match serde_json::to_value(&event) {
                Ok(payload) => {
                    let _ = app_handle.emit("session-event", &payload);
                }
                Err(e) => {
                    eprintln!("Failed to serialize DomainEvent: {e}");
                }
            }
        }
    })
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p agent-gui-tauri`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src-tauri/src/app_state.rs apps/agent-gui/src-tauri/src/event_forwarder.rs
git commit -m "feat(gui): add GuiState and event forwarder module"
```

---

## Task 3: Create Tauri Command Handlers

**Files:**

- Create: `apps/agent-gui/src-tauri/src/commands.rs`

- [ ] **Step 1: Write command handlers**

Create `apps/agent-gui/src-tauri/src/commands.rs`:

```rust
use crate::app_state::{GuiState, WorkspaceSession};
use crate::event_forwarder::spawn_event_forwarder;
use agent_core::projection::SessionProjection;
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use serde::{Deserialize, Serialize};
use tauri::State;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfoResponse {
    pub workspace_id: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfoResponse {
    pub id: String,
    pub title: String,
    pub profile: String,
}

fn detect_profiles() -> Vec<String> {
    let mut profiles = vec!["fake".to_string()];
    if std::env::var("OPENAI_API_KEY").is_ok() {
        profiles.insert(0, "fast".to_string());
    }
    profiles.insert(
        if profiles.len() > 1 { 1 } else { 0 },
        "local-code".to_string(),
    );
    profiles
}

fn choose_default_profile(profiles: &[String]) -> &str {
    if profiles.iter().any(|p| p == "fast") {
        "fast"
    } else if profiles.iter().any(|p| p == "local-code") {
        "local-code"
    } else {
        "fake"
    }
}

#[tauri::command]
pub async fn list_profiles() -> Vec<String> {
    detect_profiles()
}

#[tauri::command]
pub async fn initialize_workspace(
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<WorkspaceInfoResponse, String> {
    // Prevent double initialization
    {
        let ws = state.workspace_id.lock().await;
        if ws.is_some() {
            return Err("Workspace already initialized".into());
        }
    }

    let workspace_path = std::env::current_dir()
        .map_err(|e| format!("Cannot get current directory: {e}"))?
        .display()
        .to_string();

    let workspace = state
        .runtime
        .open_workspace(workspace_path)
        .await
        .map_err(|e| format!("Failed to open workspace: {e}"))?;

    let workspace_id = workspace.workspace_id.clone();
    let profiles = detect_profiles();
    let profile = choose_default_profile(&profiles);

    let session_id = state
        .runtime
        .start_session(agent_core::StartSessionRequest {
            workspace_id: workspace_id.clone(),
            model_profile: profile.to_string(),
        })
        .await
        .map_err(|e| format!("Failed to start session: {e}"))?;

    // Store workspace and session info
    {
        let mut ws = state.workspace_id.lock().await;
        *ws = Some(workspace_id.clone());
    }
    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(
            session_id.to_string(),
            WorkspaceSession {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                profile: profile.to_string(),
            },
        );
    }
    {
        let mut current = state.current_session_id.lock().await;
        *current = Some(session_id.clone());
    }

    // Spawn event forwarder for the initial session
    {
        let mut handle = state.forwarder_handle.lock().await;
        *handle = Some(spawn_event_forwarder(
            &state.runtime,
            session_id.clone(),
            app_handle,
        ));
    }

    Ok(WorkspaceInfoResponse {
        workspace_id: workspace_id.to_string(),
        path: workspace.path,
    })
}

#[tauri::command]
pub async fn start_session(
    profile: String,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<SessionInfoResponse, String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };

    let session_id = state
        .runtime
        .start_session(agent_core::StartSessionRequest {
            workspace_id: workspace_id.clone(),
            model_profile: profile.clone(),
        })
        .await
        .map_err(|e| format!("Failed to start session: {e}"))?;

    let title = format!("Session using {profile}");

    // Register session
    {
        let mut sessions = state.sessions.lock().await;
        sessions.insert(
            session_id.to_string(),
            WorkspaceSession {
                workspace_id,
                session_id: session_id.clone(),
                profile: profile.clone(),
            },
        );
    }

    // Switch to the new session
    switch_session_inner(&state, session_id.clone(), &app_handle).await?;

    Ok(SessionInfoResponse {
        id: session_id.to_string(),
        title,
        profile,
    })
}

#[tauri::command]
pub async fn send_message(content: String, state: State<'_, GuiState>) -> Result<(), String> {
    let workspace_id = {
        let ws = state.workspace_id.lock().await;
        ws.clone().ok_or("Workspace not initialized")?
    };
    let session_id = {
        let current = state.current_session_id.lock().await;
        current.clone().ok_or("No active session")?
    };

    state
        .runtime
        .send_message(agent_core::SendMessageRequest {
            workspace_id,
            session_id,
            content,
        })
        .await
        .map_err(|e| format!("Failed to send message: {e}"))?;

    Ok(())
}

#[tauri::command]
pub async fn switch_session(
    session_id: String,
    state: State<'_, GuiState>,
    app_handle: tauri::AppHandle,
) -> Result<SessionProjection, String> {
    let sid: agent_core::SessionId = session_id.into();
    switch_session_inner(&state, sid.clone(), &app_handle).await?;

    let projection = state
        .runtime
        .get_session_projection(sid)
        .await
        .map_err(|e| format!("Failed to get session projection: {e}"))?;

    Ok(projection)
}

#[tauri::command]
pub async fn list_sessions(
    state: State<'_, GuiState>,
) -> Result<Vec<SessionInfoResponse>, String> {
    let sessions = state.sessions.lock().await;
    let current_session_id = state.current_session_id.lock().await;

    let mut result: Vec<SessionInfoResponse> = sessions
        .values()
        .map(|s| SessionInfoResponse {
            id: s.session_id.to_string(),
            title: format!("Session using {}", s.profile),
            profile: s.profile.clone(),
        })
        .collect();

    // Sort: current session first
    if let Some(current_id) = current_session_id.as_ref() {
        result.sort_by(|a, b| {
            if a.id == current_id.to_string() {
                std::cmp::Ordering::Less
            } else if b.id == current_id.to_string() {
                std::cmp::Ordering::Greater
            } else {
                std::cmp::Ordering::Equal
            }
        });
    }

    Ok(result)
}

/// Inner helper: abort old forwarder, spawn new one, update current session.
async fn switch_session_inner(
    state: &GuiState,
    session_id: agent_core::SessionId,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    // Abort existing forwarder
    {
        let mut handle = state.forwarder_handle.lock().await;
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    // Update current session
    {
        let mut current = state.current_session_id.lock().await;
        *current = Some(session_id.clone());
    }

    // Spawn new forwarder for the target session
    {
        let mut handle = state.forwarder_handle.lock().await;
        *handle = Some(spawn_event_forwarder(
            &state.runtime,
            session_id,
            app_handle.clone(),
        ));
    }

    Ok(())
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p agent-gui-tauri`
Expected: compiles with no errors

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src-tauri/src/commands.rs
git commit -m "feat(gui): add Tauri command handlers for workspace, session, and chat"
```

---

## Task 4: Rewrite lib.rs with Full Setup and Command Registration

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/lib.rs`

- [ ] **Step 1: Replace lib.rs**

Replace `apps/agent-gui/src-tauri/src/lib.rs` with:

```rust
mod app_state;
mod commands;
mod event_forwarder;

use app_state::GuiState;
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;

pub fn build_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory();
    let model = FakeModelClient::new(vec!["hello from Kairox".into()]);

    let rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let store = rt.block_on(store).expect("failed to create event store");

    let workspace_path =
        std::env::current_dir().expect("cannot determine current directory for workspace");

    let rt2 = tokio::runtime::Runtime::new().expect("failed to create tokio runtime");
    let runtime = rt2
        .block_on(async {
            LocalRuntime::new(store, model)
                .with_permission_mode(PermissionMode::Suggest)
                .with_context_limit(100_000)
                .with_builtin_tools(workspace_path)
                .await
        })
        .expect("failed to build runtime");

    runtime
}

#[cfg(not(test))]
pub fn run() {
    let runtime = build_runtime();

    tauri::Builder::default()
        .manage(GuiState::new(runtime))
        .invoke_handler(tauri::generate_handler![
            commands::list_profiles,
            commands::initialize_workspace,
            commands::start_session,
            commands::send_message,
            commands::switch_session,
            commands::list_sessions,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}

#[cfg(test)]
pub fn run() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_profiles_returns_at_least_fake() {
        let profiles = commands::detect_profiles();
        assert!(profiles.contains(&"fake".to_string()));
    }

    #[test]
    fn choose_default_profile_picks_fast_if_available() {
        let profiles = vec!["fast".to_string(), "fake".to_string()];
        assert_eq!(commands::choose_default_profile(&profiles), "fast");
    }

    #[test]
    fn choose_default_profile_falls_back_to_fake() {
        let profiles = vec!["fake".to_string()];
        assert_eq!(commands::choose_default_profile(&profiles), "fake");
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p agent-gui-tauri`
Expected: compiles with no errors

- [ ] **Step 3: Run Tauri backend tests**

Run: `cargo test -p agent-gui-tauri`
Expected: 3 tests pass

- [ ] **Step 4: Verify full workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: all existing tests + 3 new tests pass

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src-tauri/src/lib.rs
git commit -m "feat(gui): wire up Tauri app with runtime state and command registration"
```

---

## Task 5: Create TypeScript Types

**Files:**

- Create: `apps/agent-gui/src/types/index.ts`

- [ ] **Step 1: Create types file**

Create `apps/agent-gui/src/types/index.ts`:

```typescript
export type ProjectedRole = "user" | "assistant";

export interface ProjectedMessage {
  role: ProjectedRole;
  content: string;
}

export interface SessionProjection {
  messages: ProjectedMessage[];
  task_titles: string[];
  token_stream: string;
  cancelled: boolean;
}

export type EventPayload =
  | { type: "WorkspaceOpened"; path: string }
  | { type: "UserMessageAdded"; message_id: string; content: string }
  | { type: "AgentTaskCreated"; task_id: string; title: string }
  | { type: "ModelTokenDelta"; delta: string }
  | {
      type: "AssistantMessageCompleted";
      message_id: string;
      content: string;
    }
  | {
      type: "ModelToolCallRequested";
      tool_call_id: string;
      tool_id: string;
    }
  | {
      type: "PermissionRequested";
      request_id: string;
      tool_id: string;
      preview: string;
    }
  | { type: "PermissionGranted"; request_id: string }
  | { type: "PermissionDenied"; request_id: string; reason: string }
  | {
      type: "ToolInvocationStarted";
      invocation_id: string;
      tool_id: string;
    }
  | {
      type: "ToolInvocationCompleted";
      invocation_id: string;
      tool_id: string;
      output_preview: string;
      exit_code: number | null;
      duration_ms: number;
      truncated: boolean;
    }
  | {
      type: "ToolInvocationFailed";
      invocation_id: string;
      tool_id: string;
      error: string;
    }
  | { type: "SessionCancelled"; reason: string }
  | { type: string };

export interface DomainEvent {
  schema_version: number;
  workspace_id: string;
  session_id: string;
  timestamp: string;
  source_agent_id: string;
  privacy: string;
  event_type: string;
  payload: EventPayload;
}

export interface SessionInfoResponse {
  id: string;
  title: string;
  profile: string;
}

export interface WorkspaceInfoResponse {
  workspace_id: string;
  path: string;
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/types/index.ts
git commit -m "feat(gui): add TypeScript types for DomainEvent and session data"
```

---

## Task 6: Create Session Store with Event Projection

**Files:**

- Create: `apps/agent-gui/src/stores/session.ts`

- [ ] **Step 1: Create session store**

Create `apps/agent-gui/src/stores/session.ts`:

```typescript
import { reactive } from "vue";
import type {
  SessionProjection,
  SessionInfoResponse,
  DomainEvent
} from "../types";

export const sessionState = reactive({
  sessions: [] as SessionInfoResponse[],
  currentSessionId: null as string | null,
  projection: {
    messages: [],
    task_titles: [],
    token_stream: "",
    cancelled: false
  } as SessionProjection,
  currentProfile: "fake",
  isStreaming: false,
  connected: false,
  initialized: false
});

/**
 * Apply a DomainEvent to the local session projection.
 * Mirrors the Rust SessionProjection::apply() method.
 */
export function applyEvent(event: DomainEvent) {
  const p = event.payload;
  switch (p.type) {
    case "UserMessageAdded":
      sessionState.projection.messages.push({
        role: "user",
        content: p.content
      });
      sessionState.isStreaming = true;
      break;
    case "ModelTokenDelta":
      sessionState.projection.token_stream += p.delta;
      break;
    case "AssistantMessageCompleted":
      sessionState.projection.messages.push({
        role: "assistant",
        content: p.content
      });
      sessionState.projection.token_stream = "";
      sessionState.isStreaming = false;
      break;
    case "SessionCancelled":
      sessionState.projection.cancelled = true;
      sessionState.isStreaming = false;
      break;
    case "AgentTaskCreated":
      sessionState.projection.task_titles.push(p.title);
      break;
    case "ToolInvocationStarted":
    case "ToolInvocationCompleted":
    case "ToolInvocationFailed":
    case "PermissionRequested":
    case "PermissionGranted":
    case "PermissionDenied":
      // Trace/permission events — stored but not rendered in MVP
      break;
  }
}

/**
 * Replace the current projection entirely (used after session switch).
 */
export function setProjection(projection: SessionProjection) {
  sessionState.projection = projection;
  sessionState.isStreaming = false;
}

/**
 * Reset projection to empty state (used before switching sessions).
 */
export function resetProjection() {
  sessionState.projection = {
    messages: [],
    task_titles: [],
    token_stream: "",
    cancelled: false
  };
  sessionState.isStreaming = false;
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/stores/session.ts
git commit -m "feat(gui): add reactive session store with event projection"
```

---

## Task 7: Create Tauri Events Composable

**Files:**

- Create: `apps/agent-gui/src/composables/useTauriEvents.ts`

- [ ] **Step 1: Create composable**

Create `apps/agent-gui/src/composables/useTauriEvents.ts`:

```typescript
import { onMounted, onUnmounted } from "vue";
import { listen } from "@tauri-apps/api/event";
import type { DomainEvent } from "../types";
import { sessionState, applyEvent } from "../stores/session";

export function useTauriEvents() {
  let unlisten: (() => void) | null = null;

  onMounted(async () => {
    unlisten = await listen<DomainEvent>("session-event", (event) => {
      applyEvent(event.payload);
    });
    sessionState.connected = true;
  });

  onUnmounted(() => {
    unlisten?.();
    sessionState.connected = false;
  });
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/composables/useTauriEvents.ts
git commit -m "feat(gui): add Tauri event listener composable"
```

---

## Task 8: Create ChatPanel Vue Component

**Files:**

- Create: `apps/agent-gui/src/components/ChatPanel.vue`

- [ ] **Step 1: Create ChatPanel**

Create `apps/agent-gui/src/components/ChatPanel.vue`:

```vue
<script setup lang="ts">
import { ref, nextTick, watch } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { sessionState } from "../stores/session";

const inputText = ref("");
const messageList = ref<HTMLElement | null>(null);

async function sendMessage() {
  const content = inputText.value.trim();
  if (!content || sessionState.isStreaming) return;

  inputText.value = "";
  try {
    await invoke("send_message", { content });
  } catch (e) {
    console.error("Failed to send message:", e);
  }
}

function handleKeydown(e: KeyboardEvent) {
  if (e.key === "Enter" && !e.shiftKey) {
    e.preventDefault();
    sendMessage();
  }
}

// Auto-scroll when new messages or streaming tokens arrive
watch(
  () => [
    sessionState.projection.messages.length,
    sessionState.projection.token_stream
  ],
  async () => {
    await nextTick();
    if (messageList.value) {
      messageList.value.scrollTop = messageList.value.scrollHeight;
    }
  }
);
</script>

<template>
  <section class="chat-panel">
    <header class="chat-header">
      <h2>Chat</h2>
      <span class="profile-badge">{{ sessionState.currentProfile }}</span>
    </header>
    <div ref="messageList" class="message-list">
      <div
        v-for="(msg, i) in sessionState.projection.messages"
        :key="i"
        :class="[
          'message',
          msg.role === 'user' ? 'message-user' : 'message-assistant'
        ]"
      >
        <span class="message-role">{{
          msg.role === "user" ? "You" : "Agent"
        }}</span>
        <span class="message-content">{{ msg.content }}</span>
      </div>
      <div
        v-if="sessionState.projection.token_stream"
        class="message message-assistant streaming"
      >
        <span class="message-role">Agent</span>
        <span class="message-content"
          >{{ sessionState.projection.token_stream
          }}<span class="cursor">▌</span></span
        >
      </div>
      <div v-if="sessionState.projection.cancelled" class="cancelled-marker">
        [cancelled]
      </div>
    </div>
    <div class="input-area">
      <textarea
        v-model="inputText"
        :disabled="sessionState.isStreaming"
        class="message-input"
        placeholder="Type your message..."
        rows="1"
        @keydown="handleKeydown"
      ></textarea>
      <button
        class="send-button"
        :disabled="!inputText.trim() || sessionState.isStreaming"
        @click="sendMessage"
      >
        Send
      </button>
    </div>
  </section>
</template>

<style scoped>
.chat-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.chat-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 16px;
  border-bottom: 1px solid #d7d7d7;
}
.chat-header h2 {
  margin: 0;
  font-size: 14px;
}
.profile-badge {
  font-size: 11px;
  padding: 2px 8px;
  background: #e8e8e8;
  border-radius: 4px;
  color: #555;
}
.message-list {
  flex: 1;
  overflow-y: auto;
  padding: 12px 16px;
}
.message {
  margin-bottom: 12px;
  line-height: 1.5;
}
.message-user .message-role {
  color: #0077cc;
  font-weight: 600;
}
.message-assistant .message-role {
  color: #22a06b;
  font-weight: 600;
}
.message-role {
  margin-right: 6px;
}
.message-content {
  white-space: pre-wrap;
  word-break: break-word;
}
.streaming .cursor {
  animation: blink 1s step-end infinite;
}
.cancelled-marker {
  color: #b45309;
  font-style: italic;
  margin-top: 4px;
}
@keyframes blink {
  50% {
    opacity: 0;
  }
}
.input-area {
  display: flex;
  gap: 8px;
  padding: 8px 16px;
  border-top: 1px solid #d7d7d7;
}
.message-input {
  flex: 1;
  padding: 8px;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  font-family: inherit;
  font-size: 13px;
  resize: none;
}
.message-input:disabled {
  background: #f5f5f5;
}
.send-button {
  padding: 8px 16px;
  background: #0077cc;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
  font-size: 13px;
}
.send-button:disabled {
  background: #a0c4e8;
  cursor: not-allowed;
}
</style>
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/components/ChatPanel.vue
git commit -m "feat(gui): add ChatPanel with streaming messages and input area"
```

---

## Task 9: Create SessionsSidebar Vue Component

**Files:**

- Create: `apps/agent-gui/src/components/SessionsSidebar.vue`

- [ ] **Step 1: Create SessionsSidebar**

Create `apps/agent-gui/src/components/SessionsSidebar.vue`:

```vue
<script setup lang="ts">
import { ref } from "vue";
import { invoke } from "@tauri-apps/api/core";
import type { SessionProjection } from "../types";
import {
  sessionState,
  setProjection,
  resetProjection
} from "../stores/session";

const showNewSession = ref(false);
const selectedProfile = ref("fake");

async function refreshSessions() {
  try {
    sessionState.sessions = await invoke("list_sessions");
  } catch (e) {
    console.error("Failed to list sessions:", e);
  }
}

async function switchToSession(sessionId: string) {
  try {
    resetProjection();
    const projection: SessionProjection = await invoke("switch_session", {
      sessionId
    });
    setProjection(projection);
    sessionState.currentSessionId = sessionId;
  } catch (e) {
    console.error("Failed to switch session:", e);
  }
}

async function createSession() {
  try {
    await invoke("start_session", { profile: selectedProfile.value });
    await refreshSessions();
    showNewSession.value = false;
  } catch (e) {
    console.error("Failed to start session:", e);
  }
}

async function loadProfiles() {
  try {
    const profiles: string[] = await invoke("list_profiles");
    if (profiles.length > 0) {
      selectedProfile.value = profiles[0];
    }
  } catch (e) {
    console.error("Failed to load profiles:", e);
  }
}

function openNewSessionDialog() {
  loadProfiles();
  showNewSession.value = true;
}
</script>

<template>
  <aside class="sessions-sidebar">
    <header class="sidebar-header">
      <h2>Sessions</h2>
      <button class="new-session-btn" @click="openNewSessionDialog">
        + New
      </button>
    </header>

    <ul class="session-list" v-if="sessionState.sessions.length > 0">
      <li
        v-for="session in sessionState.sessions"
        :key="session.id"
        :class="[
          'session-item',
          { active: session.id === sessionState.currentSessionId }
        ]"
        @click="switchToSession(session.id)"
      >
        <span class="session-indicator">●</span>
        <span class="session-title">{{ session.title }}</span>
      </li>
    </ul>
    <p v-else class="empty-hint">No sessions yet</p>

    <dialog v-if="showNewSession" class="new-session-dialog" open>
      <h3>New Session</h3>
      <label>
        Profile:
        <select v-model="selectedProfile">
          <option value="fake">fake (Testing)</option>
          <option value="fast">fast (OpenAI)</option>
          <option value="local-code">local-code (Ollama)</option>
        </select>
      </label>
      <div class="dialog-actions">
        <button @click="createSession">Create</button>
        <button @click="showNewSession = false">Cancel</button>
      </div>
    </dialog>
  </aside>
</template>

<style scoped>
.sessions-sidebar {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}
.sidebar-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 8px 12px;
  border-bottom: 1px solid #d7d7d7;
}
.sidebar-header h2 {
  margin: 0;
  font-size: 14px;
}
.new-session-btn {
  font-size: 12px;
  padding: 2px 8px;
  background: #0077cc;
  color: white;
  border: none;
  border-radius: 4px;
  cursor: pointer;
}
.session-list {
  list-style: none;
  padding: 0;
  margin: 0;
}
.session-item {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 8px 12px;
  cursor: pointer;
  font-size: 13px;
}
.session-item:hover {
  background: #f0f4f8;
}
.session-item.active {
  background: #e1ecf7;
  font-weight: 600;
}
.session-indicator {
  color: #22a06b;
  font-size: 10px;
}
.session-title {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.empty-hint {
  padding: 12px;
  color: #999;
  font-size: 13px;
}
.new-session-dialog {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  background: white;
  border: 1px solid #d7d7d7;
  border-radius: 8px;
  padding: 20px;
  z-index: 100;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
}
.new-session-dialog h3 {
  margin: 0 0 12px;
}
.new-session-dialog label {
  display: block;
  margin-bottom: 12px;
  font-size: 13px;
}
.new-session-dialog select {
  margin-left: 8px;
  padding: 4px;
}
.dialog-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}
.dialog-actions button {
  padding: 6px 12px;
  border: 1px solid #d7d7d7;
  border-radius: 4px;
  cursor: pointer;
  background: white;
}
.dialog-actions button:first-child {
  background: #0077cc;
  color: white;
  border-color: #0077cc;
}
</style>
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/components/SessionsSidebar.vue
git commit -m "feat(gui): add SessionsSidebar with session list and creation dialog"
```

---

## Task 10: Create StatusBar Vue Component

**Files:**

- Create: `apps/agent-gui/src/components/StatusBar.vue`

- [ ] **Step 1: Create StatusBar**

Create `apps/agent-gui/src/components/StatusBar.vue`:

```vue
<script setup lang="ts">
import { sessionState } from "../stores/session";
</script>

<template>
  <footer class="status-bar">
    <span class="status-item">profile: {{ sessionState.currentProfile }}</span>
    <span class="status-divider">│</span>
    <span class="status-item"
      >sessions: {{ sessionState.sessions.length }}</span
    >
    <span class="status-divider">│</span>
    <span class="status-item">{{
      sessionState.isStreaming ? "streaming..." : "idle"
    }}</span>
    <span class="status-divider">│</span>
    <span class="status-item">{{
      sessionState.connected ? "connected" : "disconnected"
    }}</span>
  </footer>
</template>

<style scoped>
.status-bar {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 4px 16px;
  background: #f5f5f5;
  border-top: 1px solid #d7d7d7;
  font-size: 11px;
  color: #555;
}
.status-divider {
  color: #ccc;
}
</style>
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src/components/StatusBar.vue
git commit -m "feat(gui): add StatusBar with profile, session, and connection info"
```

---

## Task 11: Update App.vue and main.ts with Real Data Wiring

**Files:**

- Modify: `apps/agent-gui/src/App.vue`
- Modify: `apps/agent-gui/src/main.ts`
- Modify: `apps/agent-gui/src/components/TraceTimeline.vue`
- Modify: `apps/agent-gui/src/components/TraceTimeline.ts`
- Modify: `apps/agent-gui/src/components/PermissionCenter.vue`

- [ ] **Step 1: Update App.vue**

Replace `apps/agent-gui/src/App.vue` with:

```vue
<script setup lang="ts">
import { onMounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { useTauriEvents } from "./composables/useTauriEvents";
import { sessionState } from "./stores/session";
import ChatPanel from "./components/ChatPanel.vue";
import SessionsSidebar from "./components/SessionsSidebar.vue";
import StatusBar from "./components/StatusBar.vue";
import TraceTimeline from "./components/TraceTimeline.vue";
import PermissionCenter from "./components/PermissionCenter.vue";

useTauriEvents();

onMounted(async () => {
  try {
    await invoke("initialize_workspace");
    sessionState.initialized = true;
    // Load initial sessions list
    sessionState.sessions = await invoke("list_sessions");
    if (sessionState.sessions.length > 0) {
      sessionState.currentSessionId = sessionState.sessions[0].id;
      sessionState.currentProfile = sessionState.sessions[0].profile;
    }
  } catch (e) {
    console.error("Failed to initialize workspace:", e);
  }
});
</script>

<template>
  <main class="workbench">
    <SessionsSidebar />
    <ChatPanel />
    <aside class="right-sidebar">
      <TraceTimeline />
      <PermissionCenter />
    </aside>
  </main>
  <StatusBar />
</template>

<style scoped>
.workbench {
  display: grid;
  grid-template-columns: 220px 1fr 280px;
  flex: 1;
  overflow: hidden;
}
.right-sidebar {
  display: flex;
  flex-direction: column;
  border-left: 1px solid #d7d7d7;
  overflow: hidden;
}
</style>
```

- [ ] **Step 2: Update main.ts**

Replace `apps/agent-gui/src/main.ts` with:

```typescript
import { createApp } from "vue";
import App from "./App.vue";

const app = createApp(App);

app.mount("#app");
```

- [ ] **Step 3: Update TraceTimeline.vue to placeholder**

Replace `apps/agent-gui/src/components/TraceTimeline.vue` with:

```vue
<template>
  <section class="trace-placeholder">
    <h2>Trace</h2>
    <p class="coming-soon">Coming soon in v0.6.0</p>
  </section>
</template>

<style scoped>
.trace-placeholder {
  padding: 16px;
  border-bottom: 1px solid #d7d7d7;
}
.trace-placeholder h2 {
  margin: 0 0 8px;
  font-size: 14px;
}
.coming-soon {
  color: #999;
  font-size: 13px;
}
</style>
```

- [ ] **Step 4: Simplify TraceTimeline.ts**

Replace `apps/agent-gui/src/components/TraceTimeline.ts` with:

```typescript
// Trace timeline types — feature coming in v0.6.0
// This file is kept for forward compatibility.
```

- [ ] **Step 5: Update TraceTimeline.test.ts**

Replace `apps/agent-gui/src/components/TraceTimeline.test.ts` with:

```typescript
import { describe, it, expect } from "vitest";

describe("TraceTimeline placeholder", () => {
  it("exists as a module placeholder", () => {
    expect(true).toBe(true);
  });
});
```

- [ ] **Step 6: Update PermissionCenter.vue to placeholder**

Replace `apps/agent-gui/src/components/PermissionCenter.vue` with:

```vue
<template>
  <section class="permission-placeholder">
    <h2>Permissions</h2>
    <p class="coming-soon">Coming soon in v0.6.0</p>
  </section>
</template>

<style scoped>
.permission-placeholder {
  padding: 16px;
}
.permission-placeholder h2 {
  margin: 0 0 8px;
  font-size: 14px;
}
.coming-soon {
  color: #999;
  font-size: 13px;
}
</style>
```

- [ ] **Step 7: Verify build**

Run: `pnpm --filter agent-gui run build`
Expected: Vite build succeeds

- [ ] **Step 8: Commit**

```bash
git add apps/agent-gui/src/
git commit -m "feat(gui): wire App.vue with real data, replace placeholders for Trace and Permission"
```

---

## Task 12: Add CSS Reset and Global Styles

**Files:**

- Modify: `apps/agent-gui/src/App.vue` (style block)
- Create: `apps/agent-gui/src/assets/main.css`

- [ ] **Step 1: Create global styles**

Create `apps/agent-gui/src/assets/main.css`:

```css
*,
*::before,
*::after {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
}

html,
body,
#app {
  height: 100%;
  width: 100%;
  overflow: hidden;
  font-family:
    system-ui,
    -apple-system,
    "Segoe UI",
    Roboto,
    sans-serif;
  font-size: 14px;
  color: #333;
  background: #fff;
}

#app {
  display: flex;
  flex-direction: column;
}
```

- [ ] **Step 2: Import global styles in main.ts**

Update `apps/agent-gui/src/main.ts` to:

```typescript
import { createApp } from "vue";
import App from "./App.vue";
import "./assets/main.css";

const app = createApp(App);

app.mount("#app");
```

- [ ] **Step 3: Verify build**

Run: `pnpm --filter agent-gui run build`
Expected: build succeeds

- [ ] **Step 4: Commit**

```bash
git add apps/agent-gui/src/assets/main.css apps/agent-gui/src/main.ts
git commit -m "feat(gui): add global CSS reset and layout styles"
```

---

## Task 13: Add Tauri Capabilities for Event Emission

Tauri 2 requires explicit capability permissions for IPC. We need to allow `emit` from the backend and `invoke` from the frontend.

**Files:**

- Create: `apps/agent-gui/src-tauri/capabilities/default.json`

- [ ] **Step 1: Create capabilities file**

Create `apps/agent-gui/src-tauri/capabilities/default.json`:

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default capabilities for the main window",
  "windows": ["main"],
  "permissions": [
    "core:default",
    "core:event:default",
    "core:event:allow-emit",
    "core:event:allow-listen"
  ]
}
```

- [ ] **Step 2: Commit**

```bash
git add apps/agent-gui/src-tauri/capabilities/
git commit -m "feat(gui): add Tauri capabilities for event emission and listening"
```

---

## Task 14: Integration Test — Rust Command Handlers

**Files:**

- Modify: `apps/agent-gui/src-tauri/src/lib.rs` (add test module)

- [ ] **Step 1: Add integration tests to lib.rs**

Add to `apps/agent-gui/src-tauri/src/lib.rs` test module:

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use agent_core::AppFacade;

    async fn create_test_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["test response".into()]);
        LocalRuntime::new(store, model)
            .with_permission_mode(PermissionMode::Suggest)
            .with_context_limit(100_000)
    }

    #[tokio::test]
    async fn workspace_initialization_creates_session() {
        let runtime = create_test_runtime().await;
        let workspace = runtime
            .open_workspace("/tmp/test".into())
            .await
            .unwrap();

        let session_id = runtime
            .start_session(agent_core::StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        assert!(!session_id.to_string().is_empty());
    }

    #[tokio::test]
    async fn send_message_produces_user_and_assistant_events() {
        let runtime = create_test_runtime().await;
        let workspace = runtime
            .open_workspace("/tmp/test".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(agent_core::StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hello".into(),
            })
            .await
            .unwrap();

        let projection = runtime.get_session_projection(session_id).await.unwrap();
        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].content, "hello");
        assert_eq!(projection.messages[1].content, "test response");
    }

    #[tokio::test]
    async fn session_projection_serializes_for_frontend() {
        let runtime = create_test_runtime().await;
        let workspace = runtime
            .open_workspace("/tmp/test".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(agent_core::StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hi".into(),
            })
            .await
            .unwrap();

        let projection = runtime.get_session_projection(session_id).await.unwrap();

        // Verify it serializes to JSON that the frontend can parse
        let json = serde_json::to_value(&projection).unwrap();
        assert!(json["messages"].is_array());
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][1]["role"], "assistant");
    }

    #[tokio::test]
    async fn domain_event_serializes_with_payload_type_tag() {
        let runtime = create_test_runtime().await;
        let workspace = runtime
            .open_workspace("/tmp/test".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(agent_core::StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "test".into(),
            })
            .await
            .unwrap();

        // Subscribe and verify event structure
        let mut stream = runtime.subscribe_session(session_id);
        let events: Vec<agent_core::DomainEvent> = {
            use futures::StreamExt;
            let mut collected = Vec::new();
            for _ in 0..10 {
                tokio::select! {
                    Some(event) = stream.next() => collected.push(event),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => break,
                }
            }
            collected
        };

        assert!(!events.is_empty());
        for event in &events {
            let json = serde_json::to_value(event).unwrap();
            assert!(json["payload"]["type"].is_string());
            assert!(json["session_id"].is_string());
        }
    }
}
```

- [ ] **Step 2: Update lib.rs module with agent_models and futures imports**

Add to the top of `lib.rs`:

```rust
#[cfg(test)]
use agent_models::FakeModelClient;
#[cfg(test)]
use agent_store::SqliteEventStore;
#[cfg(test)]
use agent_tools::PermissionMode;
```

- [ ] **Step 3: Run Tauri backend tests**

Run: `cargo test -p agent-gui-tauri`
Expected: all tests pass (3 unit + 4 integration)

- [ ] **Step 4: Run full workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add apps/agent-gui/src-tauri/src/lib.rs
git commit -m "test(gui): add integration tests for command handlers and event serialization"
```

---

## Task 15: Frontend Unit Tests

**Files:**

- Create: `apps/agent-gui/src/stores/session.test.ts`
- Create: `apps/agent-gui/src/composables/useTauriEvents.test.ts`

- [ ] **Step 1: Create session store test**

Create `apps/agent-gui/src/stores/session.test.ts`:

```typescript
import { describe, it, expect, beforeEach } from "vitest";
import {
  sessionState,
  applyEvent,
  setProjection,
  resetProjection
} from "./session";
import type { DomainEvent } from "../types";

// Reset state between tests
beforeEach(() => {
  sessionState.sessions = [];
  sessionState.currentSessionId = null;
  sessionState.isStreaming = false;
  sessionState.connected = false;
  resetProjection();
});

describe("applyEvent", () => {
  it("projects UserMessageAdded", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "UserMessageAdded",
      payload: { type: "UserMessageAdded", message_id: "m1", content: "hello" }
    } as DomainEvent);

    expect(sessionState.projection.messages).toHaveLength(1);
    expect(sessionState.projection.messages[0].role).toBe("user");
    expect(sessionState.projection.messages[0].content).toBe("hello");
    expect(sessionState.isStreaming).toBe(true);
  });

  it("accumulates ModelTokenDelta into token_stream", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "ModelTokenDelta",
      payload: { type: "ModelTokenDelta", delta: "hel" }
    } as DomainEvent);
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:01Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "ModelTokenDelta",
      payload: { type: "ModelTokenDelta", delta: "lo" }
    } as DomainEvent);

    expect(sessionState.projection.token_stream).toBe("hello");
  });

  it("finalizes on AssistantMessageCompleted", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "AssistantMessageCompleted",
      payload: {
        type: "AssistantMessageCompleted",
        message_id: "m2",
        content: "hi there"
      }
    } as DomainEvent);

    expect(sessionState.projection.messages).toHaveLength(1);
    expect(sessionState.projection.messages[0].role).toBe("assistant");
    expect(sessionState.projection.messages[0].content).toBe("hi there");
    expect(sessionState.projection.token_stream).toBe("");
    expect(sessionState.isStreaming).toBe(false);
  });

  it("marks cancelled on SessionCancelled", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "SessionCancelled",
      payload: { type: "SessionCancelled", reason: "user stopped" }
    } as DomainEvent);

    expect(sessionState.projection.cancelled).toBe(true);
    expect(sessionState.isStreaming).toBe(false);
  });

  it("ignores unknown event types gracefully", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "FutureEvent",
      payload: { type: "FutureEvent" }
    } as DomainEvent);

    expect(sessionState.projection.messages).toHaveLength(0);
  });
});

describe("setProjection", () => {
  it("replaces the current projection", () => {
    setProjection({
      messages: [
        { role: "user", content: "existing" },
        { role: "assistant", content: "reply" }
      ],
      task_titles: ["task 1"],
      token_stream: "",
      cancelled: false
    });

    expect(sessionState.projection.messages).toHaveLength(2);
    expect(sessionState.isStreaming).toBe(false);
  });
});

describe("resetProjection", () => {
  it("clears all projection state", () => {
    applyEvent({
      schema_version: 1,
      workspace_id: "wrk_1",
      session_id: "ses_1",
      timestamp: "2026-05-01T00:00:00Z",
      source_agent_id: "agent_system",
      privacy: "full_trace",
      event_type: "UserMessageAdded",
      payload: { type: "UserMessageAdded", message_id: "m1", content: "hi" }
    } as DomainEvent);

    resetProjection();

    expect(sessionState.projection.messages).toHaveLength(0);
    expect(sessionState.projection.token_stream).toBe("");
    expect(sessionState.projection.cancelled).toBe(false);
    expect(sessionState.isStreaming).toBe(false);
  });
});
```

- [ ] **Step 2: Run frontend tests**

Run: `pnpm --filter agent-gui run test`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add apps/agent-gui/src/stores/session.test.ts
git commit -m "test(gui): add session store unit tests for event projection"
```

---

## Task 16: Final Verification and Cleanup

**Files:**

- Potentially all modified files for minor fixes

- [ ] **Step 1: Run Rust workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: all tests pass

- [ ] **Step 2: Run frontend lint and format checks**

Run: `pnpm run format:check && pnpm run lint`
Expected: no errors

- [ ] **Step 3: Run Rust format and clippy**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings

- [ ] **Step 4: Verify GUI builds**

Run: `pnpm --filter agent-gui run build`
Expected: Vite build succeeds

- [ ] **Step 5: Manual smoke test with Tauri dev**

Run: `pnpm --filter agent-gui run tauri:dev`
Expected: Kairox window opens, shows three-column layout, chat input works, FakeModelClient responds with streaming output

- [ ] **Step 6: Commit any remaining fixes**

```bash
git add -A
git commit -m "chore(gui): final verification and cleanup for GUI integration MVP"
```
