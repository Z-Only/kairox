# GUI Integration Design — Progressive MVP

Date: 2026-05-01
Status: Draft
Scope: `apps/agent-gui` (Tauri 2 + Vue 3), `crates/agent-core`, `crates/agent-runtime`

## Context

Kairox v0.4.0 has a fully functional interactive TUI (three-panel layout, streaming chat, permission prompts, session management, trace panel) but the GUI (`apps/agent-gui`) is a shell — a single `list_model_profiles` Tauri command and a static Vue layout with placeholder components. The ROADMAP explicitly lists "Improve desktop GUI beyond the current shell" as the top near-term priority.

This design defines a **Progressive MVP** that gets the desktop app to a usable state: live chat with streaming output, session management, and model profile switching. Trace panel and permission interaction are deferred to the next iteration.

## Goals

1. Bridge the Rust `AppFacade` trait to the Vue frontend via Tauri 2 commands and events
2. Implement a Chat panel with streaming model output in the GUI
3. Implement a Sessions sidebar with session list, creation, and switching
4. Implement model profile selection on session creation
5. Illuminate the existing three-column layout with real data and interactivity

## Non-Goals (Deferred to v0.6.0)

- Trace panel with L1/L2/L3 density views
- Permission prompts (inline and modal) — in Suggest mode, auto-deny writes; in Autonomous mode, auto-allow
- Multi-agent orchestration UI
- MCP configuration UI
- Memory editor UI
- Settings / configuration file UI
- Keymap customization
- Workspace-switching (only current directory)
- Offline / reconnection handling

## Architecture

### Data Flow

```
Vue Frontend                        Rust Backend (Tauri)
─────────────                       ─────────────────────
                                    LocalRuntime (AppFacade)
                                        │
invoke("open_workspace")  ──────►   open_workspace()
invoke("start_session")   ──────►   start_session()
invoke("send_message")    ──────►   send_message()
invoke("list_sessions")   ──────►   get_session_projection()
                                        │
listen("session-event")  ◄──────   app.emit("session-event", DomainEvent)
                                        │
                                    DomainEvent pipeline:
                                    UserMessageAdded → ModelTokenDelta →
                                    AssistantMessageCompleted → ToolInvocation* → ...
```

### Key Decisions

| Decision          | Choice                                                                                              | Rationale                                                  |
| ----------------- | --------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| Event transport   | Tauri `app.emit()` → Vue `listen()`                                                                 | Standard Tauri 2 pattern; low latency; no WebSocket needed |
| State management  | Vue `reactive()` stores                                                                             | Simple for MVP; no Pinia overhead yet                      |
| Session tracking  | HashMap in Tauri state                                                                              | Rust side owns runtime; sessions keyed by SessionId        |
| Profile detection | Reuse `detect_profiles()` logic from TUI                                                            | Consistent behavior between TUI and GUI                    |
| Permission mode   | Default to `Suggest` (auto-deny writes)                                                             | Safe MVP default; no UI for permission prompts yet         |
| Streaming render  | Accumulate `ModelTokenDelta` in Vue reactive state; tokenize rendering with `requestAnimationFrame` | Smooth visual updates without per-token reflows            |

### Tauri State Management

A single `AppState` struct managed by Tauri's state mechanism:

```rust
// apps/agent-gui/src-tauri/src/app_state.rs
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_models::FakeModelClient;
use agent_core::{SessionId, WorkspaceId};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

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
}
```

> **Note:** For MVP, `FakeModelClient` is the only model adapter wired in the Tauri backend. OpenAI and Ollama adapters exist in `agent-models` but require config/API key management that belongs in the configuration system (v0.6.0). The profile selector shows `fake`, and `detect_profiles()` will add `fast` and `local-code` when env vars are present.

## Tauri Commands

New commands to add to `apps/agent-gui/src-tauri/src/`:

### `initialize_workspace`

```rust
#[tauri::command]
async fn initialize_workspace(state: tauri::State<'_, GuiState>) -> Result<WorkspaceInfo, String> {
    // Opens current directory as workspace
    // Starts a default session with the first available model profile
    // Spawns a background task to forward DomainEvents via app.emit()
}
```

### `list_profiles`

```rust
#[tauri::command]
async fn list_profiles() -> Vec<String> {
    // Detects available model profiles (same logic as TUI)
}
```

### `start_session`

```rust
#[tauri::command]
async fn start_session(
    profile: String,
    state: tauri::State<'_, GuiState>,
) -> Result<SessionInfo, String> {
    // Creates a new session with the given profile
    // Returns session metadata for the frontend
    // Spawns event forwarding for the new session
}
```

### `send_message`

```rust
#[tauri::command]
async fn send_message(
    content: String,
    state: tauri::State<'_, GuiState>,
) -> Result<(), String> {
    // Sends user message to current session
    // Events flow back via session-event channel
}
```

### `switch_session`

```rust
#[tauri::command]
async fn switch_session(
    session_id: String,
    state: tauri::State<'_, GuiState>,
) -> Result<SessionProjection, String> {
    // Switches active session, returns current projection for replay
}
```

### `get_session_projection`

```rust
#[tauri::command]
async fn get_session_projection(
    session_id: String,
    state: tauri::State<'_, GuiState>,
) -> Result<SessionProjection, String> {
    // Returns current session state for session switching/reconnection
}
```

### Event Forwarding

After `initialize_workspace`, a background tokio task subscribes to `subscribe_session()` and forwards each `DomainEvent` to the frontend:

```rust
let mut rx = runtime.subscribe_session(session_id);
let app_handle = app_handle.clone();
tokio::spawn(async move {
    while let Some(event) = rx.next().await {
        let _ = app_handle.emit("session-event", &event);
    }
});
```

## Vue Frontend

### File Structure

```
apps/agent-gui/src/
├── App.vue                    # Root layout (updated)
├── main.ts                    # Entry point (updated)
├── stores/
│   └── session.ts             # Reactive session state
├── composables/
│   └── useTauriEvents.ts      # Event listener lifecycle
├── components/
│   ├── ChatPanel.vue          # Message list + input (new)
│   ├── SessionsSidebar.vue    # Session list + new session (new)
│   ├── StatusBar.vue          # Profile, session info, hints (new)
│   ├── TraceTimeline.vue      # Placeholder (existing, keep minimal)
│   └── PermissionCenter.vue   # Placeholder (existing, keep minimal)
└── types/
    └── index.ts               # TypeScript types for DomainEvent, SessionProjection, etc.
```

### Types (`types/index.ts`)

TypeScript mirrors of Rust types, derived from the `Serialize` impls:

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
  | { type: "UserMessageAdded"; message_id: string; content: string }
  | { type: "ModelTokenDelta"; delta: string }
  | { type: "AssistantMessageCompleted"; message_id: string; content: string }
  | { type: "ToolInvocationStarted"; invocation_id: string; tool_id: string }
  | {
      type: "ToolInvocationCompleted";
      invocation_id: string;
      tool_id: string;
      output_preview: string;
      exit_code: number | null;
      duration_ms: number;
      truncated: boolean;
    }
  | { type: "PermissionGranted"; request_id: string }
  | { type: "PermissionDenied"; request_id: string; reason: string }
  | { type: "SessionCancelled"; reason: string }
  | { type: "WorkspaceOpened"; path: string }
  | { type: "AgentTaskCreated"; task_id: string; title: string }
  | { type: string }; // fallback for unknown events

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

export interface SessionInfo {
  id: string;
  title: string;
  profile: string;
  active: boolean;
}
```

### Session Store (`stores/session.ts`)

```typescript
import { reactive, computed } from "vue";
import type { SessionProjection, SessionInfo, DomainEvent } from "../types";

export const sessionState = reactive({
  sessions: [] as SessionInfo[],
  currentSessionId: null as string | null,
  projection: {
    messages: [],
    task_titles: [],
    token_stream: "",
    cancelled: false
  } as SessionProjection,
  currentProfile: "fake",
  isStreaming: false,
  connected: false
});

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
    case "ToolInvocationStarted":
    case "ToolInvocationCompleted":
      // Trace events — stored but not rendered in MVP
      break;
    case "AgentTaskCreated":
      sessionState.projection.task_titles.push(p.title);
      break;
  }
}

export function resetProjection() {
  sessionState.projection = {
    messages: [],
    task_titles: [],
    token_stream: "",
    cancelled: false
  };
}
```

> **Design note:** The `applyEvent` function mirrors the TUI's `SessionProjection::apply()` method. This is intentional — both UIs project from the same event stream. If event schemas change, both need updating, which is acceptable for MVP. A shared projection library (e.g., compiled to WASM) could be a future improvement.

### Tauri Events Composable (`composables/useTauriEvents.ts`)

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

### ChatPanel.vue (New)

The core chat component. Renders messages, streaming tokens, and provides the input area.

**Features:**

- Message list: scrolls to bottom on new messages/tokens
- Streaming indicator: shows `token_stream` content with a blinking cursor
- Cancelled marker: shows `[cancelled]` badge
- Input area: single-line input, Enter sends, Shift+Enter for newline
- Auto-scroll: `scrollIntoView` on new content
- Loading state: disabled input while `isStreaming`
- Error display: inline error messages below the message list

**Layout:**

```
┌─────────────────────────────────────┐
│  Chat                    [profile]  │
├─────────────────────────────────────┤
│                                     │
│  You: fix the build error           │
│  Agent: I found the issue in        │
│  Cargo.toml...▌                     │
│                                     │
│                                     │
├─────────────────────────────────────┤
│  > type your message...      [Send] │
└─────────────────────────────────────┘
```

### SessionsSidebar.vue (New)

Session list and new-session creation.

**Features:**

- Session list with active indicator (● green, ○ gray)
- Click to switch session (invokes `switch_session` command)
- "New Session" button with profile selector
- Current session highlighted

**Layout:**

```
┌──────────────┐
│  Sessions     │
│  [+ New]      │
│               │
│  ● Session 1  │
│  ○ Session 2  │
│  ○ Session 3  │
└──────────────┘
```

### StatusBar.vue (New)

Fixed bottom bar showing current state.

**Layout:**

```
 profile: fake │ sessions: 3 │ streaming: no │ connected: yes
```

### App.vue (Updated)

Three-column layout with real data:

```
┌──────────┬──────────────────────┬──────────────┐
│ Sessions │       Chat           │    Trace     │
│ Sidebar  │                      │  (deferred)  │
│          │                      │              │
│          │                      │  Coming soon │
│          │                      │              │
├──────────┴──────────────────────┴──────────────┤
│                 Status Bar                      │
└─────────────────────────────────────────────────┘
```

The Trace column shows a "Coming soon" placeholder. PermissionCenter is removed from the layout for MVP.

## Rust Backend Changes

### File Structure

```
apps/agent-gui/src-tauri/src/
├── lib.rs              # Updated: Tauri setup, state init, command registration
├── commands.rs         # New: all Tauri command handlers
├── app_state.rs        # New: GuiState struct
└── event_forwarder.rs  # New: DomainEvent → Tauri event bridge
```

### `lib.rs` Changes

Replace the current `commands` module and `run()` function with:

1. Build `SqliteEventStore` (in-memory for MVP)
2. Build `LocalRuntime` with `FakeModelClient` and builtin tools
3. Wrap in `GuiState`
4. Register all Tauri commands
5. On window creation, auto-call `initialize_workspace`

### `event_forwarder.rs`

Spawns a tokio task per session that subscribes to `subscribe_session()` and emits `DomainEvent`s to the frontend via `app_handle.emit()`.

Key consideration: when the user switches sessions, we need to:

1. Unsubscribe from the old session's event stream
2. Subscribe to the new session
3. Fetch the new session's projection via `get_session_projection` for initial state

This is managed by storing the `JoinHandle` of the forwarding task in `GuiState` and aborting it on session switch.

### Dependency Changes

```toml
# apps/agent-gui/src-tauri/Cargo.toml additions
[dependencies]
agent-core = { path = "../../../crates/agent-core" }
agent-models = { path = "../../../crates/agent-models" }
agent-runtime = { path = "../../../crates/agent-runtime" }
agent-store = { path = "../../../crates/agent-store" }
agent-tools = { path = "../../../crates/agent-tools" }
tokio = { workspace = true }
futures = { workspace = true }
serde_json = { workspace = true }
```

The Tauri crate already depends on `agent-core` and `serde`/`serde_json`. We add the full runtime stack.

## Session Lifecycle

### Initialization Flow

```
App mounts
  → invoke("initialize_workspace")
    → Rust: open_workspace(cwd), start_session(default_profile)
    → Rust: spawn event_forwarder for session
    → Rust: emit("session-event", WorkspaceOpened)
    → Rust: emit("session-event", AgentTaskCreated)
  → Vue: sessionState updated, ChatPanel shows empty session
```

### Chat Flow

```
User types message, presses Enter
  → invoke("send_message", { content })
    → Rust: runtime.send_message(request)
    → Rust: runtime appends UserMessageAdded → emit
    → Rust: model loop generates tokens → emit ModelTokenDelta per token
    → Rust: emit AssistantMessageCompleted
  → Vue: applyEvent() updates projection reactively
  → Vue: ChatPanel auto-scrolls, shows streaming cursor
```

### Session Switch Flow

```
User clicks session in sidebar
  → invoke("switch_session", { session_id })
    → Rust: abort old event_forwarder task
    → Rust: subscribe_session(new_session_id), spawn new forwarder
    → Rust: return projection of new session
  → Vue: resetProjection(), load returned projection
  → Vue: ChatPanel re-renders with session messages
```

### New Session Flow

```
User clicks "New Session", selects profile
  → invoke("start_session", { profile })
    → Rust: runtime.start_session(request)
    → Rust: spawn event_forwarder for new session
    → Rust: emit AgentTaskCreated
  → Vue: add session to list, auto-switch to new session
```

## Testing Strategy

### Rust Unit Tests

- Each Tauri command tested independently with `FakeModelClient`
- `initialize_workspace` returns valid workspace info
- `start_session` creates session and spawns forwarder
- `send_message` produces correct event sequence
- `switch_session` aborts old forwarder, subscribes to new session
- Event payload serialization round-trips correctly

### Vue Component Tests (Vitest)

- `ChatPanel.vue`: renders messages, shows streaming cursor, sends on Enter
- `SessionsSidebar.vue`: renders session list, click triggers switch
- `StatusBar.vue`: shows profile and connection state
- `applyEvent()`: correctly projects all event types
- `useTauriEvents()`: lifecycle mount/unmount

### Integration Test (Manual)

- `pnpm --filter agent-gui run tauri:dev` opens app
- User can type messages and see FakeModelClient responses
- Streaming tokens appear with cursor animation
- Session switching preserves message history
- New session creation works with profile selection
- Window resize maintains layout

## Acceptance Criteria

The MVP is complete when:

1. `pnpm --filter agent-gui run tauri:dev` opens the Kairox GUI with a three-column layout
2. ChatPanel displays a message input that sends messages via Tauri command
3. Streaming model output (ModelTokenDelta events) renders incrementally with a blinking cursor
4. Completed assistant messages appear as chat bubbles
5. SessionsSidebar shows the active session and a "New Session" button
6. Clicking "New Session" with a profile creates and switches to a new session
7. Switching between sessions loads the correct message history
8. StatusBar displays the current profile name and session count
9. The existing `cargo test --workspace` suite passes with no regressions
10. The GUI builds on macOS (Tauri dev mode); Linux/Windows builds are not blocked

## Version Target

This design targets **v0.5.0** of Kairox.
