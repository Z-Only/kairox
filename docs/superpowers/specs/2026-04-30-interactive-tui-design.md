# Interactive ratatui TUI Design

Date: 2026-04-30
Status: Approved
Scope: `agent-tui` crate — full interactive terminal UI for Kairox

## Context

Kairox v0.3.0 has a working runtime (agent loop, tool dispatch, permissions, event broadcast, model adapters) but the `agent-tui` crate only prints projection output to stdout and exits. The ratatui dependency is declared but never used for rendering. The design spec (M1 milestone) requires an interactive TUI with chat, trace, permission prompts, model selection, and session management.

This design replaces the current `main.rs` (CLI print-and-exit) with a full interactive ratatui application while preserving the existing `view.rs` and `app.rs` as initial building blocks.

## Goals

1. Implement a three-panel interactive TUI layout with toggleable sidebars
2. Wire the `AppFacade` trait (open_workspace, start_session, send_message, decide_permission, cancel_session, subscribe_session) into the TUI event loop
3. Support streaming model output with adaptive frame-rate rendering
4. Implement permission interaction (inline prompts + modal for destructive operations)
5. Support single-line / multi-line input with auto-upgrade
6. Enable model profile switching and session management
7. Establish a Component trait architecture that scales to future panels without degrading into a monolith

## Non-Goals

- Multi-agent orchestration UI (M5 milestone)
- MCP configuration UI (M4 milestone)
- Memory editor UI (M4 milestone)
- Account/login UI (M6 milestone)
- Full Markdown rendering (terminal Markdown is limited; basic code block and list support is in scope)
- Terminal multiplexer compatibility beyond documented limitations

---

## Layout

### Three-Panel Architecture

```
┌──────────────┬────────────────────────────┬──────────────────┐
│  Sessions    │         Chat               │     Trace        │
│  (toggle)    │                            │    (toggle)      │
│              │  You: fix the build error  │  ▶ shell.exec ✓  │
│  ● main      │  Agent: I found the issue  │  ▶ fs.read ✓    │
│  ○ refactor  │  in Cargo.toml...          │  ▶ patch.apply ⏳│
│  ○ debug     │                            │                  │
│              │  ▌streaming...             │  ⚠ write perm    │
│              │                            │  [Y] [N]         │
│              ├────────────────────────────┤                  │
│              │  > type your message...    │                  │
└──────────────┴────────────────────────────┴──────────────────┘
├────────────────────── Status Bar ────────────────────────────┤
│  profile: fast  │  mode: suggest  │  sessions: 3  │ Alt+S/? │
└──────────────────────────────────────────────────────────────┘
```

Both sidebars are independently toggleable:

- **Left sidebar hidden**: Chat expands left; Trace stays right
- **Right sidebar hidden**: Chat expands right; Sessions stays left
- **Both hidden**: Chat takes full width (maximum readability for long conversations)

Toggle shortcuts: `Alt+S` (Sessions), `Alt+T` (Trace).

### Sidebar Widths

| Panel      | Default Width     | Collapsed |
| ---------- | ----------------- | --------- |
| Sessions   | 24 chars          | 0         |
| Trace      | 32 chars          | 0         |
| Chat       | remaining space   | —         |
| Status bar | full width, 1 row | —         |

Minimum terminal size: 80×24. Below this, show a warning and refuse to render.

---

## Panel Specifications

### Chat Panel (Center)

The primary interaction surface. Contains:

1. **Message area** (scrollable): Renders `SessionProjection.messages` as chat bubbles
   - User messages: `You: <content>`
   - Assistant messages: `Agent: <content>`
   - Streaming text: displayed with blinking block cursor `▌`
   - Interrupted text: preserved with `[cancelled]` marker

2. **Input area** (bottom of panel, fixed height):
   - Single-line mode: one line, Enter sends, ↑↓ history
   - Multi-line mode: 3-line editor, Enter adds newline, Ctrl+Enter sends
   - Auto-upgrade: pasting content with newlines switches to multi-line
   - Permission-wait state: input area replaced with `[Y] Allow  [N] Deny  [D] Deny all` prompt

3. **Streaming token renderer**:
   - Tokens accumulate in `AppState.token_stream`
   - Rendering is frame-rate throttled (adaptive, 60ms base interval)
   - Markdown-aware: code blocks (` ``` `), headings, and lists are flushed at semantic boundaries
   - On interruption (Ctrl+C in PermissionMode::Suggest or Agent mode), emit `cancel_session` and mark output as cancelled

### Sessions Panel (Left Sidebar)

Lists all sessions for the current workspace:

- **Session states**: Active (● green), Idle (○ gray), Error (✕ red), Awaiting Permission (⚠ yellow)
- **Pin support**: Pinned sessions float to top with 📌 indicator
- **Operations**: New (Alt+N), Switch (Enter), Rename (F2), Delete, Pin/Unpin, Search/Filter
- **Context menu**: Press `x` to open operation menu for selected session
- **Profile tags**: Each session shows its model profile label (e.g., `[fast]`, `[local-code]`)
- **New session flow**: Triggers profile selection popup, then creates session

### Trace Panel (Right Sidebar)

Three-layer information density:

**L1 — Default Summary**: One line per tool invocation

```
▶ shell.exec  ✓  1.2s
▶ fs.read     ✓  0.3s
▶ patch.apply ⏳
```

**L2 — Expanded Detail**: Press Enter on a trace entry to expand

```
▶ patch.apply ⏳
  args: { path: "src/main.rs", ... }
  output: --- a/src/main.rs +++ b/src/main.rs ...
```

**L3 — Full Event Stream**: Press F5 to toggle; shows all event types

```
[12:01:03] UserMessageAdded    "fix the build error"
[12:01:03] ModelTokenDelta     "I found..."
[12:01:04] ToolCallRequested   shell.exec
[12:01:04] PermissionGranted   shell.exec
[12:01:05] ToolInvocationCompleted shell.exec ✓ 1.2s
[12:01:05] AssistantMessageCompleted "The issue is..."
```

### Status Bar (Bottom)

Fixed 1-row bar spanning full width:

```
 profile: fast │ mode: suggest │ sessions: 3 │ Alt+S/? for help
```

Fields:

- Current model profile name
- Current permission mode
- Active session count
- Context-sensitive hint (changes based on focus state)

---

## Permission Interaction

Permission prompts integrate with the existing `PermissionEngine` and `ToolRisk`/`ToolEffect` system.

### Interaction Mode by Risk Level

| Risk Level              | ToolEffect Examples                        | Interaction                 |
| ----------------------- | ------------------------------------------ | --------------------------- |
| Read                    | `fs.read`, `search.ripgrep`                | Auto-allowed, no prompt     |
| Write / Shell / Network | `shell.exec` (Write), `patch.apply`        | Inline prompt in chat       |
| Destructive             | `shell.exec` (Destructive: rm, sudo, mkfs) | Modal dialog (must confirm) |

### Inline Permission Prompt

Rendered as a special message in the chat area:

```
⚠ shell.exec: cargo test (write)
[Y] Allow  [N] Deny  [D] Deny all similar
```

Input area is temporarily replaced with shortcut keys. After response:

- `Y` → `decide_permission(approved: true)`, tool executes
- `N` → `decide_permission(approved: false)`, tool skipped
- `D` → deny + add temporary rule to auto-deny similar calls this session

### Destructive Operation Modal

Full-screen centered modal:

```
┌─────────────────────────────────────────┐
│  ⛔ Destructive Operation               │
│                                         │
│  Tool: shell.exec                       │
│  Command: rm -rf target/                │
│  Risk: Destructive                      │
│                                         │
│  This operation cannot be undone.       │
│                                         │
│  [Y] Allow once  [N] Deny  [Esc] Cancel │
└─────────────────────────────────────────┘
```

Focus is captured by the modal; all other key bindings are suppressed until dismissed.

---

## Input System

### Modes

| Mode        | Enter        | Navigate            | Switch              |
| ----------- | ------------ | ------------------- | ------------------- |
| Single-line | Send message | ↑↓ history          | Alt+E → multi-line  |
| Multi-line  | New line     | ↑↓ scroll           | Alt+E → single-line |
|             | Ctrl+Enter   | Send                |                     |
|             | Esc (empty)  | Back to single-line |                     |

### Auto-Upgrade

When the terminal receives a `Paste` event containing newlines, automatically switch to multi-line mode. This is detected via crossterm's `EventKind::Paste` or multi-char key sequences.

### Command History

Single-line mode maintains a per-session command history (stored in `AppState`). ↑ scrolls back, ↓ scrolls forward, matching the pattern used by shells.

### Permission-Wait State

When a permission request is pending, the input area is completely replaced:

```
⚠ shell.exec: cargo build (write)  [Y] Allow  [N] Deny  [D] Deny all
```

Normal input is suspended; only Y/N/D/Esc are processed. After resolution, the input area is restored with its previous content.

---

## Model Profile Switching

### Status Bar Indicator

The status bar always shows the current profile name. `Alt+P` opens a profile selection popup from anywhere.

### Profile Selection Popup

```
┌──────────────────────────┐
│  Select Model Profile    │
│                          │
│  > fast     (OpenAI)     │
│    local-code (Ollama)   │
│    fake     (Testing)    │
│                          │
│  Enter to select, Esc    │
└──────────────────────────┘
```

- Profiles are detected at startup via `detect_profiles()` (checking `OPENAI_API_KEY`, Ollama availability)
- Selecting a profile starts a **new session** with that profile (does not switch the existing session's model)
- Each session in the Sessions panel shows its profile tag
- Profile configuration file loading is deferred to the config management task

---

## Keybinding System

### Layering Rules

Keys are assigned by **operation frequency × safety**:

| Layer            | Modifier    | Frequency | Purpose                       | Examples                                                               |
| ---------------- | ----------- | --------- | ----------------------------- | ---------------------------------------------------------------------- |
| L1 Instant       | None        | ★★★★★     | In-session highest frequency  | Enter, Esc, Tab, ↑↓, Y/N, x                                            |
| L2 Alt Modifier  | Option/Alt  | ★★★★      | Panel and mode switching      | Alt+S, Alt+T, Alt+E, Alt+P, Alt+N, Alt+Q                               |
| L3 Ctrl Reserved | Ctrl        | ★★★       | Terminal convention keys only | Ctrl+C (interrupt/exit), Ctrl+L (redraw), Ctrl+Enter (send multi-line) |
| L4 Function      | F1-F12      | ★★        | Low-frequency functions       | F1 (help), F2 (rename), F5 (trace view toggle)                         |
| L5 Custom        | keymap.toml | ★         | User overrides                | Override any L1-L4 binding                                             |

### Cross-Platform Modifier Mapping

| Layer | macOS     | Linux  | Windows | Notes                                                    |
| ----- | --------- | ------ | ------- | -------------------------------------------------------- |
| L2    | ⌥ Option  | Alt    | Alt     | macOS Terminal.app requires "Use Option as Meta" setting |
| L3    | ⌃ Control | Ctrl   | Ctrl    | Captured in raw mode; only 3 safe convention keys        |
| L4    | Fn+F1-F12 | F1-F12 | F1-F12  | macOS requires Fn key for function row                   |

### Complete Key Map

**L1 Instant Keys (no modifier):**

| Key    | Action                                        | Context             |
| ------ | --------------------------------------------- | ------------------- |
| Enter  | Send (single-line) / Expand detail (Trace L2) | Input / Trace focus |
| Escape | Back / Close popup / Exit multi-line          | Global              |
| Tab    | Focus cycle: Chat → Sessions → Trace          | Global              |
| ↑ / ↓  | History (input) / Navigate (panel)            | By context          |
| Y      | Allow permission                              | Permission-wait     |
| N      | Deny permission                               | Permission-wait     |
| D      | Deny all similar this session                 | Permission-wait     |
| x      | Context menu                                  | Panel focus         |

**L2 Alt Modifier:**

| Key   | Action                                | Context    |
| ----- | ------------------------------------- | ---------- |
| Alt+S | Toggle Sessions sidebar               | Global     |
| Alt+T | Toggle Trace sidebar                  | Global     |
| Alt+E | Toggle single-line / multi-line input | Input area |
| Alt+P | Profile selection popup               | Global     |
| Alt+N | New session                           | Global     |
| Alt+Q | Quit Kairox (with confirmation)       | Global     |
| Alt+1 | Focus Chat panel                      | Global     |
| Alt+2 | Focus Sessions panel                  | Global     |
| Alt+3 | Focus Trace panel                     | Global     |

**L3 Ctrl Reserved:**

| Key                     | Action                             | Context    |
| ----------------------- | ---------------------------------- | ---------- |
| Ctrl+C (1st)            | Interrupt current Agent generation | Global     |
| Ctrl+C (2nd)            | Exit confirmation: "Quit? Y/N"     | Global     |
| Ctrl+C (3rd, within 2s) | Force quit without confirmation    | Global     |
| Ctrl+L                  | Redraw / clear screen              | Global     |
| Ctrl+Enter              | Send message (multi-line mode)     | Input area |

**L4 Function Keys:**

| Key | Action                                   | Context        |
| --- | ---------------------------------------- | -------------- |
| F1  | Help / keybinding reference              | Global         |
| F2  | Rename selected session                  | Sessions focus |
| F5  | Toggle Trace view density (L1 → L2 → L3) | Trace focus    |

**L5 Custom (keymap.toml):**

Users can override any L1-L4 binding via a configuration file. The file format and loading mechanism will be specified in the config management task. When custom bindings conflict with layer rules, L5 always wins.

---

## Component Architecture

### Component Trait

```rust
/// A self-contained UI panel that handles events and renders itself.
///
/// Components never directly reference other components.
/// Cross-panel communication flows exclusively through `CrossPanelEffect`
/// routed by the App layer.
trait Component {
    /// Process an incoming event. Returns effects that need cross-panel routing.
    fn handle_event(&mut self, ctx: &EventContext, event: &Event) -> Vec<CrossPanelEffect>;

    /// Receive a cross-panel effect dispatched by the App layer.
    fn handle_effect(&mut self, effect: &CrossPanelEffect);

    /// Render this component into the given area.
    fn render(&self, area: Rect, frame: &mut Frame);

    /// Whether this component currently holds focus.
    fn focused(&self) -> bool;

    /// Set focus state (for highlight rendering).
    fn set_focused(&mut self, focused: bool);
}
```

### Cross-Panel Effects

Panel-to-panel communication is mediated through a closed set of effects:

```rust
/// A permission request to be presented to the user.
struct PermissionRequest {
    request_id: String,
    tool_id: String,
    tool_preview: String,     // e.g., "shell.exec: cargo test"
    risk_level: RiskLevel,    // Write or Destructive
}

enum RiskLevel {
    Write,
    Destructive,
}

/// Summary info for the status bar.
struct StatusInfo {
    profile: String,
    permission_mode: String,
    session_count: usize,
    hint: String,
    error: Option<String>,
}

/// A session visible in the sidebar.
struct SessionInfo {
    id: SessionId,
    title: String,
    model_profile: String,
    state: SessionState,
    pinned: bool,
}

enum SessionState {
    Active,
    Idle,
    Error(String),
    AwaitingPermission,
}

enum CrossPanelEffect {
    SwitchFocus(FocusTarget),
    ShowPermissionPrompt(PermissionRequest),
    DismissPermissionPrompt,
    UpdateSessionList(Vec<SessionInfo>),
    SetStatus(StatusInfo),
    NavigateToSession(SessionId),
    StartStreaming,
    StopStreaming,
}
```

Rules:

- Components return `Vec<CrossPanelEffect>` from `handle_event`
- App collects effects, mutates `AppState`, then calls `handle_effect` on target components
- Components never import or reference other component types

### Shared State vs Component-Local State

| State                                | Owner                     | Access                                                                    |
| ------------------------------------ | ------------------------- | ------------------------------------------------------------------------- |
| Session list, messages, trace events | `AppState` (App)          | Read via `EventContext`, write via `CrossPanelEffect`                     |
| Focus target                         | `FocusManager` (App)      | `CrossPanelEffect::SwitchFocus`                                           |
| Input content, cursor position       | `ChatPanel` (local)       | Never exposed                                                             |
| Trace expand/collapse state          | `TracePanel` (local)      | Never exposed                                                             |
| Sessions selected index, scroll      | `SessionsPanel` (local)   | Never exposed                                                             |
| Permission modal state               | `PermissionModal` (local) | Never exposed                                                             |
| Inline permission pending state      | `ChatPanel` (local)       | Never exposed; ChatPanel renders inline Y/N/D as part of its message area |
| Sidebar visibility                   | `AppState` (App)          | `Alt+S` / `Alt+T` toggle                                                  |

### EventContext (Read-Only Shared State)

```rust
struct EventContext<'a> {
    focus: FocusTarget,
    current_session: &'a SessionProjection,
    sessions: &'a [SessionInfo],
    model_profile: &'a str,
    permission_mode: PermissionMode,
    sidebar_left_visible: bool,
    sidebar_right_visible: bool,
}
```

Components read shared state through `ctx` and request changes through `CrossPanelEffect`.

### FocusManager

Focus is managed exclusively by the App layer using a stack:

```rust
struct FocusManager {
    stack: Vec<FocusTarget>,
}

impl FocusManager {
    fn current(&self) -> FocusTarget;
    fn push(&mut self, target: FocusTarget);   // Modal takes over
    fn pop(&mut self) -> Option<FocusTarget>;  // Modal dismissed
    fn cycle_next(&mut self);                  // Tab cycling
}

#[derive(Clone, Copy, PartialEq)]
enum FocusTarget {
    Chat,
    Sessions,
    Trace,
    PermissionModal,
}
```

When a destructive operation modal appears, `PermissionModal` is pushed onto the stack. When dismissed, the previous focus target is restored.

---

## Runtime Event Integration

### Event Bridge: DomainEvent → AppEvent

The `subscribe_session` stream is async; the TUI event loop must merge terminal events and runtime events:

```rust
enum AppEvent {
    Key(KeyEvent),
    DomainEvent(DomainEvent),
    Tick,
}
```

A tokio task forwards runtime events into a channel:

```rust
let tx = app_event_tx.clone();
tokio::spawn(async move {
    let mut stream = runtime.subscribe_session(session_id);
    while let Some(event) = stream.next().await {
        tx.send(AppEvent::DomainEvent(event)).await.ok();
    }
});
```

The main loop merges all sources:

```rust
loop {
    tokio::select! {
        Some(event) = app_event_rx.recv() => app.handle(event),
    }
    if render_scheduler.should_render() {
        app.render(terminal);
    }
}
```

### DomainEvent Processing

DomainEvents update `AppState` and produce `CrossPanelEffect`s as needed:

| DomainEvent                 | AppState Update              | CrossPanelEffect               |
| --------------------------- | ---------------------------- | ------------------------------ |
| `UserMessageAdded`          | Append to session projection | — (ChatPanel reads from ctx)   |
| `ModelTokenDelta`           | Append to token_stream       | `StartStreaming` (first token) |
| `AssistantMessageCompleted` | Finalize message             | `StopStreaming`                |
| `ToolInvocationStarted`     | Append to trace              | `SetStatus(tool_running)`      |
| `ToolInvocationCompleted`   | Update trace entry           | —                              |
| `PermissionGranted/Denied`  | Update permission state      | `DismissPermissionPrompt`      |
| `SessionCancelled`          | Mark cancelled               | `StopStreaming`                |

### Commands (App → Runtime)

User actions that invoke the runtime are represented as commands:

```rust
enum Command {
    SendMessage { workspace_id: WorkspaceId, session_id: SessionId, content: String },
    DecidePermission { request_id: String, approved: bool },
    CancelSession { workspace_id: WorkspaceId, session_id: SessionId },
    StartSession { workspace_id: WorkspaceId, model_profile: String },
}
```

Components produce `Command`s alongside `CrossPanelEffect`s:

```rust
fn handle_event(&mut self, ctx: &EventContext, event: &Event) -> (Vec<CrossPanelEffect>, Vec<Command>);
```

App dispatches commands to `tokio::spawn` tasks that call the `AppFacade`. Results flow back through `DomainEvent`.

---

## Render Scheduler

Token-by-token rendering would cause excessive redraws. The render scheduler throttles frame production to balance responsiveness and performance.

### Ownership

`RenderScheduler` is owned by `App` and consulted in the main event loop after each event processing cycle. It does not own any rendering logic — it only decides **when** to call `App.render()`.

```rust
struct RenderScheduler {
    last_render: Instant,
    min_interval: Duration,       // Adaptive: 16ms–120ms
    dirty: bool,
    is_streaming: bool,           // True while receiving ModelTokenDelta
    token_count_since_render: usize,
    streaming_start: Option<Instant>,
}
```

### API

| Method                    | Description                                                                                         |
| ------------------------- | --------------------------------------------------------------------------------------------------- |
| `mark_dirty()`            | Mark state as changed; render will occur on next eligible tick                                      |
| `mark_dirty_immediate()`  | Force next tick to render regardless of throttle (used for key presses, resize, permission prompts) |
| `set_streaming(bool)`     | Enter/exit streaming mode; resets adaptive counters                                                 |
| `should_render() -> bool` | Check if enough time has elapsed and state is dirty; reset dirty flag                               |
| `reset()`                 | Reset all counters (called on session switch or after interruption)                                 |

### Adaptive Interval Calculation

The `min_interval` adjusts dynamically based on token arrival rate during streaming:

```
if is_streaming:
    if token_count_since_render >= 20:
        min_interval = 120ms   // High throughput: batch more tokens per frame
    elif token_count_since_render >= 5:
        min_interval = 60ms    // Moderate throughput: default streaming rate
    else:
        min_interval = 16ms    // Low throughput: near-instant feedback
else:
    min_interval = 16ms        // Not streaming: respond immediately to all changes
```

After each render, `token_count_since_render` resets to 0.

### Event-to-Scheduler Mapping

| Event Source                                 | Scheduler Call                                    | Behavior                                              |
| -------------------------------------------- | ------------------------------------------------- | ----------------------------------------------------- |
| `ModelTokenDelta`                            | `mark_dirty()`                                    | Does not render immediately; token counter increments |
| Key press, resize, permission prompt         | `mark_dirty_immediate()`                          | Bypasses throttle; renders on next tick               |
| `AssistantMessageCompleted`, `StopStreaming` | `mark_dirty_immediate()` + `set_streaming(false)` | Final render at full speed                            |
| `SessionCancelled`                           | `mark_dirty_immediate()` + `reset()`              | Full reset, immediate render                          |
| Session switch                               | `reset()` + `mark_dirty_immediate()`              | Clear counters, force full redraw                     |
| `Tick` (~16ms interval)                      | Checked by event loop                             | Calls `should_render()` to decide if render occurs    |

---

## App Lifecycle

### Startup

1. Detect terminal size; if below 80×24, print error message to stderr and exit with code 1
2. Initialize `SqliteEventStore` (in-memory or file-backed based on config)
3. Detect model profiles and select default
4. Construct `LocalRuntime` with selected model, permission mode, builtin tools
5. Open workspace (current directory)
6. Start default session
7. Enter raw mode, enable mouse capture (if desired), launch terminal event loop

Startup failure handling:

| Step                    | Failure              | Response                                                                                  |
| ----------------------- | -------------------- | ----------------------------------------------------------------------------------------- |
| 1. Terminal size        | < 80×24              | Print `Error: Terminal too small (need 80×24, got WxH). Please resize.` to stderr, exit 1 |
| 2. Event store          | SQLite error         | Print `Error: Failed to initialize event store: <reason>` to stderr, exit 1               |
| 3. Model profiles       | No profiles detected | Fall back to `fake` profile; show warning in status bar                                   |
| 4. Runtime construction | Tool init failure    | Print warning, continue without failed tools; show in status bar                          |
| 5. Open workspace       | Permission denied    | Print `Error: Cannot open workspace at <path>: <reason>` to stderr, exit 1                |
| 6. Start session        | Runtime error        | Print `Error: Failed to start session: <reason>` to stderr, exit 1                        |

### Terminal Resize

When the terminal is resized (crossterm `Event::Resize`):

1. App recalculates layout: sidebar widths remain fixed, Chat panel takes remaining space
2. If new size falls below 80×24 **while the TUI is running**: display a centered warning overlay `⚠ Terminal too small — resize to continue`, suppress all key handling except resize events
3. When size returns to ≥ 80×24: remove overlay, resume normal rendering
4. `RenderScheduler.mark_dirty()` is called immediately on resize to force a full redraw
5. Content that no longer fits (long messages, trace entries) is scrolled to keep the cursor/input area visible

### Session Flow

**Happy path:**

```
User types message → ChatPanel.handle_event → (effects, [Command::SendMessage])
                                                  ↓
App dispatches Command → runtime.send_message()
                                                  ↓
DomainEvent stream → App.handle_domain_event → AppState update + effects
                                                  ↓
Components receive effects → render on next frame
```

**Error paths:**

| Failure Point                               | Detection                         | UI Response                                                                                                                                     |
| ------------------------------------------- | --------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| `runtime.send_message()` returns `Err`      | Command dispatch catches error    | ChatPanel displays error message: `⚠ Failed to send: <reason>`. Input preserved for retry. ChatPanel emits `SetStatus(StatusInfo::error(msg))`. |
| `runtime.start_session()` returns `Err`     | Command dispatch catches error    | SessionsPanel shows error toast: `⚠ Could not start session: <reason>`. No session created.                                                     |
| `runtime.decide_permission()` returns `Err` | Command dispatch catches error    | Permission prompt stays visible. Error shown inline or in modal: `⚠ Permission response failed: <reason>`. User can retry.                      |
| `runtime.open_workspace()` returns `Err`    | Startup failure                   | Display error in terminal before TUI init, exit with code 1 and descriptive message.                                                            |
| DomainEvent stream closes unexpectedly      | Channel receiver returns `None`   | `SetStatus(StatusInfo::error("Runtime disconnected"))`. ChatPanel appends system message: `⚠ Connection to runtime lost. Restart needed.`       |
| Model adapter returns `ModelEvent::Failed`  | DomainEvent processing            | ChatPanel displays: `⚠ Model error: <message>`. Session remains active for retry.                                                               |
| Agent loop hits `MaxIterationsExceeded`     | DomainEvent processing (existing) | ChatPanel displays: `⚠ Agent loop reached maximum iterations.` as assistant message.                                                            |

Error messages are always displayed in the chat area (as system messages) and mirrored in the status bar. The application never crashes on runtime errors — all errors are surfaced to the user with actionable context.

### Permission Flow

Permission prompts follow two distinct paths based on risk level. The focus management differs between them:

**Write/Shell/Network — Inline Path (no focus change):**

```
DomainEvent (tool call with Write/Shell/Network risk)
    → PermissionEngine decides RequiresApproval
    → App emits CrossPanelEffect::ShowPermissionPrompt(risk=Write)
    → ChatPanel renders inline prompt, replaces input area with Y/N/D keys
    → Focus STAYS on ChatPanel (no FocusManager change)
    → User presses Y/N/D
    → ChatPanel.handle_event → Command::DecidePermission
    → App dispatches Command → runtime.decide_permission()
    → DomainEvent (Granted/Denied)
    → App emits CrossPanelEffect::DismissPermissionPrompt
    → ChatPanel restores input area, resumes normal input
```

**Destructive — Modal Path (focus captured):**

```
DomainEvent (tool call with Destructive risk)
    → PermissionEngine decides RequiresApproval
    → App emits CrossPanelEffect::ShowPermissionPrompt(risk=Destructive)
    → App.focus_manager.push(FocusTarget::PermissionModal)
    → PermissionModal renders centered overlay, captures ALL key input
    → User presses Y/N/Esc
    → PermissionModal.handle_event → Command::DecidePermission
    → App dispatches Command → runtime.decide_permission()
    → DomainEvent (Granted/Denied)
    → App emits CrossPanelEffect::DismissPermissionPrompt
    → App.focus_manager.pop()  → previous focus restored
    → PermissionModal removed from render
```

Key rule: **inline prompts never change focus; only the destructive modal pushes onto the focus stack.** This ensures that multiple inline permission prompts can appear in sequence without corrupting the focus stack, while the destructive modal always gets exclusive focus.

### Shutdown

1. First `Ctrl+C`: interrupt current generation (cancel_session)
2. Second `Ctrl+C`: show "Quit Kairox? Y/N" confirmation
3. Third `Ctrl+C` within 2 seconds: force quit
4. `Alt+Q`: show quit confirmation regardless of state
5. On confirmed quit: restore terminal to cooked mode, flush event store, exit

---

## File Structure

All changes are in `crates/agent-tui/`:

```
crates/agent-tui/
├── Cargo.toml
└── src/
    ├── main.rs              # Entry point: terminal setup, event loop
    ├── app.rs               # App struct: Component routing, AppState, event handling
    ├── app_state.rs         # AppState: shared state, FocusManager, RenderScheduler
    ├── components/
    │   ├── mod.rs           # Component trait, CrossPanelEffect, FocusTarget
    │   ├── chat.rs          # ChatPanel: messages + input
    │   ├── sessions.rs      # SessionsPanel: session list + operations
    │   ├── trace.rs         # TracePanel: L1/L2/L3 trace views
    │   ├── status_bar.rs    # StatusBar: profile, mode, hints
    │   # (inline permission rendering is part of chat.rs, not a separate file)
    │   └── permission_modal.rs   # Destructive operation modal
    ├── keybindings.rs       # Key map definitions, layer resolution
    └── view.rs              # Render helpers (existing, refactored)
```

### Dependency Changes

```toml
# crates/agent-tui/Cargo.toml additions
[dependencies]
crossterm = "0.28"    # Terminal events (added alongside ratatui)
```

`ratatui` and `crossterm` are already in the workspace; `crossterm` just needs to be added to agent-tui's dependencies explicitly.

---

## Testing Strategy

### Unit Tests

- `keybindings.rs`: Key resolution per layer, platform mapping, L5 override
- `app_state.rs`: FocusManager push/pop/cycle, sidebar toggle state
- `components/chat.rs`: Input mode switching, auto-upgrade detection, history navigation
- `components/sessions.rs`: Session list ordering, Pin behavior, search filtering
- `components/trace.rs`: L1/L2/L3 view state transitions
- `view.rs`: Projection-to-rendered-lines (existing, extended)

### Component Isolation Tests

Each component is testable in isolation by constructing a minimal `EventContext` and asserting `handle_event` returns the expected `CrossPanelEffect`s. No terminal or runtime required.

### Integration Tests

- Full App with `FakeModelClient`: send message → receive response → verify events and renders
- Permission flow: FakeModelClient requests tool → permission prompt appears → user responds → tool executes
- Streaming: FakeModelClient emits tokens → verify adaptive render throttling
- Sidebar toggle: verify layout recalculation on show/hide

### Cross-Platform Keybinding Tests

The keybinding system must be verified across platforms. Since crossterm `KeyEvent` normalizes platform differences, tests operate on `KeyEvent` values directly:

- **L1 keys**: Verify `Enter`, `Esc`, `Tab`, `Up/Down`, `Y/N/D`, `x` map to correct actions in each focus context
- **L2 Alt keys**: Verify `KeyEvent { modifiers: ALT, code: Char('s') }` resolves to sidebar toggle on all platforms
- **L3 Ctrl keys**: Verify `Ctrl+C` progressive exit (1st → interrupt, 2nd → confirm, 3rd → force) with timeout reset
- **L4 function keys**: Verify `F1/F2/F5` resolve in all contexts
- **L5 override**: Verify `keymap.toml` custom binding shadows L1-L4 default
- **Paste detection**: Verify multi-line paste event triggers input mode auto-upgrade
- **Focus context switching**: Verify same key (e.g., `Enter`) routes to correct action based on `FocusTarget`

Platform-specific notes in tests:

- macOS: `Option` key produces `KeyEvent { modifiers: ALT }` when "Use Option as Meta" is enabled; document this requirement in test comments
- Windows: some `Alt+key` combos may conflict with system menu accelerators; test that fallback keys work
- All platforms: `Ctrl+C` must be caught in raw mode before SIGINT; test with simulated rapid double/triple press within 2-second window

### Snapshot Tests

Use `insta` for rendered terminal output snapshots:

- Chat panel with messages
- Trace panel at L1/L2/L3 density
- Permission modal
- Status bar with various states
- Terminal resize: layout at 80×24, 120×40, and below-minimum warning overlay

---

## Acceptance Criteria

The design is successful when:

1. `cargo run -p agent-tui` opens an interactive terminal UI that stays running until the user quits
2. Users can type messages and see streaming Agent responses
3. Tool calls appear in the Trace panel with status and timing
4. Permission prompts appear inline (Write) or as modals (Destructive) and correctly throttle tool execution
5. Users can switch model profiles and start new sessions
6. Sessions panel shows all sessions with correct state indicators
7. Both sidebars can be independently toggled with keyboard shortcuts
8. Input mode switches between single-line and multi-line, with auto-upgrade on paste
9. All keybindings work in crossterm raw mode on macOS, Linux, and Windows
10. Existing `cargo test --workspace` continues to pass with no regressions
11. **Terminal resize**: Layout recalculates correctly on resize events; below 80×24 shows warning overlay; returning to valid size restores normal rendering
12. **Ctrl+C progressive exit**: First press interrupts current generation, second press within 5 seconds shows quit confirmation, third press within 2 seconds force-quits; timeout resets the counter
13. **Error degradation**: Runtime errors (model failure, store failure, stream disconnect) surface as chat messages and status bar indicators; the TUI never crashes or becomes unresponsive due to a runtime error
14. **Permission focus integrity**: Inline permission prompts (Write/Shell/Network) do NOT change the focus stack; destructive modals push `PermissionModal` and pop it on dismissal; rapid sequential permissions don't corrupt focus state
15. **Startup failure**: Terminal too small, store init failure, or workspace open failure print descriptive error to stderr and exit with code 1 (no TUI rendered)
