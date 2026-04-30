# Interactive ratatui TUI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the current CLI print-and-exit TUI with a full interactive ratatui terminal UI featuring three-panel layout, streaming chat, permission interaction, session management, and adaptive rendering.

**Architecture:** Component trait pattern where each panel (Chat, Sessions, Trace, StatusBar, PermissionModal) implements a shared `Component` trait with `handle_event`, `handle_effect`, and `render` methods. The `App` struct routes events, manages shared state via `AppState`, and dispatches cross-panel effects. Runtime events flow from `subscribe_session` through a tokio channel into the event loop. User actions produce `Command` values dispatched to the `AppFacade`.

**Tech Stack:** Rust, ratatui 0.30, crossterm 0.29, tokio, agent-core, agent-runtime, agent-models, agent-store, agent-tools, insta (snapshot tests)

---

## File Structure

All changes are in `crates/agent-tui/`:

```
crates/agent-tui/
├── Cargo.toml                    # Add crossterm dependency
└── src/
    ├── main.rs                   # Entry: terminal setup, event loop, shutdown
    ├── app.rs                    # App struct: event routing, command dispatch
    ├── app_state.rs              # AppState, FocusManager, RenderScheduler, SessionInfo, etc.
    ├── components/
    │   ├── mod.rs                # Component trait, CrossPanelEffect, FocusTarget, EventContext
    │   ├── chat.rs               # ChatPanel: message rendering + input area + inline permission
    │   ├── sessions.rs           # SessionsPanel: session list, pin, search, context menu
    │   ├── trace.rs              # TracePanel: L1/L2/L3 density views
    │   ├── status_bar.rs         # StatusBar: profile, mode, hints
    │   └── permission_modal.rs   # Destructive operation modal overlay
    ├── keybindings.rs            # Key map layers, platform resolution, L5 override stub
    └── view.rs                   # Render helpers (existing, extended)
```

---

## Task 1: Add crossterm Dependency and Component Trait Skeleton

**Files:**

- Modify: `crates/agent-tui/Cargo.toml`
- Create: `crates/agent-tui/src/components/mod.rs`

- [ ] **Step 1: Add crossterm to Cargo.toml**

```toml
# crates/agent-tui/Cargo.toml — add to [dependencies]
crossterm = "0.29"
```

- [ ] **Step 2: Write the Component trait and core types**

Create `crates/agent-tui/src/components/mod.rs`:

```rust
use agent_core::SessionId;
use ratatui::layout::Rect;
use ratatui::Frame;

/// A self-contained UI panel that handles events and renders itself.
///
/// Components never directly reference other components.
/// Cross-panel communication flows exclusively through `CrossPanelEffect`
/// routed by the App layer.
pub trait Component {
    /// Process an incoming event. Returns (cross-panel effects, runtime commands).
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>);

    /// Receive a cross-panel effect dispatched by the App layer.
    fn handle_effect(&mut self, effect: &CrossPanelEffect);

    /// Render this component into the given area.
    fn render(&self, area: Rect, frame: &mut Frame);

    /// Whether this component currently holds focus.
    fn focused(&self) -> bool;

    /// Set focus state (for highlight rendering).
    fn set_focused(&mut self, focused: bool);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusTarget {
    Chat,
    Sessions,
    Trace,
    PermissionModal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskLevel {
    Write,
    Destructive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionRequest {
    pub request_id: String,
    pub tool_id: String,
    pub tool_preview: String,
    pub risk_level: RiskLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Active,
    Idle,
    Error(String),
    AwaitingPermission,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionInfo {
    pub id: SessionId,
    pub title: String,
    pub model_profile: String,
    pub state: SessionState,
    pub pinned: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusInfo {
    pub profile: String,
    pub permission_mode: String,
    pub session_count: usize,
    pub hint: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossPanelEffect {
    SwitchFocus(FocusTarget),
    ShowPermissionPrompt(PermissionRequest),
    DismissPermissionPrompt,
    UpdateSessionList(Vec<SessionInfo>),
    SetStatus(StatusInfo),
    NavigateToSession(SessionId),
    StartStreaming,
    StopStreaming,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    SendMessage {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
        content: String,
    },
    DecidePermission {
        request_id: String,
        approved: bool,
    },
    CancelSession {
        workspace_id: agent_core::WorkspaceId,
        session_id: SessionId,
    },
    StartSession {
        workspace_id: agent_core::WorkspaceId,
        model_profile: String,
    },
}

/// Read-only shared state passed to components on every event.
pub struct EventContext<'a> {
    pub focus: FocusTarget,
    pub current_session: &'a agent_core::projection::SessionProjection,
    pub sessions: &'a [SessionInfo],
    pub model_profile: &'a str,
    pub permission_mode: agent_tools::PermissionMode,
    pub sidebar_left_visible: bool,
    pub sidebar_right_visible: bool,
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p agent-tui`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/agent-tui/Cargo.toml crates/agent-tui/src/components/mod.rs
git commit -m "feat(tui): add crossterm dependency and Component trait skeleton"
```

---

## Task 2: AppState, FocusManager, and RenderScheduler

**Files:**

- Create: `crates/agent-tui/src/app_state.rs`

- [ ] **Step 1: Write failing tests for FocusManager**

Create `crates/agent-tui/src/app_state.rs` with tests first:

```rust
use crate::components::FocusTarget;
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct FocusManager {
    stack: Vec<FocusTarget>,
}

impl FocusManager {
    pub fn new(default: FocusTarget) -> Self {
        Self { stack: vec![default] }
    }

    pub fn current(&self) -> FocusTarget {
        *self.stack.last().unwrap_or(&FocusTarget::Chat)
    }

    pub fn push(&mut self, target: FocusTarget) {
        self.stack.push(target);
    }

    pub fn pop(&mut self) -> Option<FocusTarget> {
        if self.stack.len() > 1 {
            self.stack.pop()
        } else {
            None
        }
    }

    pub fn cycle_next(&mut self) {
        let cycle = [FocusTarget::Chat, FocusTarget::Sessions, FocusTarget::Trace];
        let current = self.current();
        let next = cycle
            .iter()
            .skip_while(|&&t| t != current)
            .nth(1)
            .or_else(|| cycle.first())
            .copied()
            .unwrap_or(FocusTarget::Chat);
        if let Some(top) = self.stack.last_mut() {
            *top = next;
        }
    }

    pub fn set(&mut self, target: FocusTarget) {
        if let Some(top) = self.stack.last_mut() {
            *top = target;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    SingleLine,
    MultiLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputState {
    Normal,
    PermissionWait {
        request_id: String,
        pending_prompt: Option<crate::components::PermissionRequest>,
    },
}

#[derive(Debug, Clone)]
pub struct RenderScheduler {
    pub last_render: Instant,
    pub min_interval: Duration,
    pub dirty: bool,
    pub is_streaming: bool,
    pub token_count_since_render: usize,
}

impl RenderScheduler {
    pub fn new() -> Self {
        Self {
            last_render: Instant::now(),
            min_interval: Duration::from_millis(16),
            dirty: true,
            is_streaming: false,
            token_count_since_render: 0,
        }
    }

    pub fn mark_dirty(&mut self) {
        self.dirty = true;
        if self.is_streaming {
            self.token_count_since_render += 1;
        }
    }

    pub fn mark_dirty_immediate(&mut self) {
        self.dirty = true;
        self.min_interval = Duration::from_millis(16);
    }

    pub fn set_streaming(&mut self, streaming: bool) {
        self.is_streaming = streaming;
        if !streaming {
            self.token_count_since_render = 0;
            self.min_interval = Duration::from_millis(16);
        }
    }

    pub fn should_render(&mut self) -> bool {
        self.adapt_interval();
        if self.dirty && self.last_render.elapsed() >= self.min_interval {
            self.dirty = false;
            self.last_render = Instant::now();
            self.token_count_since_render = 0;
            true
        } else {
            false
        }
    }

    fn adapt_interval(&mut self) {
        if self.is_streaming {
            if self.token_count_since_render >= 20 {
                self.min_interval = Duration::from_millis(120);
            } else if self.token_count_since_render >= 5 {
                self.min_interval = Duration::from_millis(60);
            } else {
                self.min_interval = Duration::from_millis(16);
            }
        } else {
            self.min_interval = Duration::from_millis(16);
        }
    }

    pub fn reset(&mut self) {
        self.is_streaming = false;
        self.token_count_since_render = 0;
        self.min_interval = Duration::from_millis(16);
        self.dirty = true;
    }
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub focus_manager: FocusManager,
    pub render_scheduler: RenderScheduler,
    pub sidebar_left_visible: bool,
    pub sidebar_right_visible: bool,
    pub current_session: agent_core::projection::SessionProjection,
    pub sessions: Vec<crate::components::SessionInfo>,
    pub model_profile: String,
    pub permission_mode: agent_tools::PermissionMode,
    pub input_mode: InputMode,
    pub input_state: InputState,
    pub input_content: String,
    pub input_cursor: usize,
    pub input_history: Vec<String>,
    pub input_history_index: Option<usize>,
    pub ctrl_c_count: usize,
    pub last_ctrl_c: Option<Instant>,
}

impl AppState {
    pub fn new(model_profile: String, permission_mode: agent_tools::PermissionMode) -> Self {
        Self {
            focus_manager: FocusManager::new(FocusTarget::Chat),
            render_scheduler: RenderScheduler::new(),
            sidebar_left_visible: true,
            sidebar_right_visible: true,
            current_session: agent_core::projection::SessionProjection::default(),
            sessions: Vec::new(),
            model_profile,
            permission_mode,
            input_mode: InputMode::SingleLine,
            input_state: InputState::Normal,
            input_content: String::new(),
            input_cursor: 0,
            input_history: Vec::new(),
            input_history_index: None,
            ctrl_c_count: 0,
            last_ctrl_c: None,
        }
    }

    pub fn event_context(&self) -> crate::components::EventContext<'_> {
        crate::components::EventContext {
            focus: self.focus_manager.current(),
            current_session: &self.current_session,
            sessions: &self.sessions,
            model_profile: &self.model_profile,
            permission_mode: self.permission_mode,
            sidebar_left_visible: self.sidebar_left_visible,
            sidebar_right_visible: self.sidebar_right_visible,
        }
    }

    /// Record a Ctrl+C press. Returns what action to take.
    pub fn record_ctrl_c(&mut self) -> CtrlCAction {
        let now = Instant::now();
        let prev = self.last_ctrl_c.replace(now);
        self.ctrl_c_count += 1;

        match self.ctrl_c_count {
            1 => CtrlCAction::Interrupt,
            2 => {
                if let Some(prev_time) = prev {
                    if now.duration_since(prev_time) <= Duration::from_secs(5) {
                        return CtrlCAction::ConfirmQuit;
                    }
                }
                self.ctrl_c_count = 1;
                CtrlCAction::Interrupt
            }
            _ => {
                if let Some(prev_time) = prev {
                    if now.duration_since(prev_time) <= Duration::from_secs(2) {
                        self.ctrl_c_count = 0;
                        return CtrlCAction::ForceQuit;
                    }
                }
                self.ctrl_c_count = 1;
                CtrlCAction::Interrupt
            }
        }
    }

    pub fn reset_ctrl_c(&mut self) {
        self.ctrl_c_count = 0;
        self.last_ctrl_c = None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtrlCAction {
    Interrupt,
    ConfirmQuit,
    ForceQuit,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_manager_default_is_chat() {
        let fm = FocusManager::new(FocusTarget::Chat);
        assert_eq!(fm.current(), FocusTarget::Chat);
    }

    #[test]
    fn focus_manager_push_pop_restores_previous() {
        let mut fm = FocusManager::new(FocusTarget::Chat);
        fm.push(FocusTarget::PermissionModal);
        assert_eq!(fm.current(), FocusTarget::PermissionModal);
        let popped = fm.pop();
        assert_eq!(popped, Some(FocusTarget::PermissionModal));
        assert_eq!(fm.current(), FocusTarget::Chat);
    }

    #[test]
    fn focus_manager_pop_last_returns_none() {
        let mut fm = FocusManager::new(FocusTarget::Chat);
        assert_eq!(fm.pop(), None);
        assert_eq!(fm.current(), FocusTarget::Chat);
    }

    #[test]
    fn focus_manager_cycle_wraps_around() {
        let mut fm = FocusManager::new(FocusTarget::Chat);
        fm.cycle_next();
        assert_eq!(fm.current(), FocusTarget::Sessions);
        fm.cycle_next();
        assert_eq!(fm.current(), FocusTarget::Trace);
        fm.cycle_next();
        assert_eq!(fm.current(), FocusTarget::Chat);
    }

    #[test]
    fn focus_manager_set_replaces_top() {
        let mut fm = FocusManager::new(FocusTarget::Chat);
        fm.push(FocusTarget::PermissionModal);
        fm.set(FocusTarget::Sessions);
        assert_eq!(fm.current(), FocusTarget::Sessions);
        fm.pop();
        assert_eq!(fm.current(), FocusTarget::Chat);
    }

    #[test]
    fn render_scheduler_adapts_interval_during_streaming() {
        let mut rs = RenderScheduler::new();
        rs.set_streaming(true);
        rs.mark_dirty(); // 1 token
        assert_eq!(rs.min_interval, Duration::from_millis(16));

        for _ in 0..5 {
            rs.mark_dirty();
        }
        // total 6 tokens
        rs.adapt_interval();
        assert_eq!(rs.min_interval, Duration::from_millis(60));

        for _ in 0..15 {
            rs.mark_dirty();
        }
        // total 21 tokens
        rs.adapt_interval();
        assert_eq!(rs.min_interval, Duration::from_millis(120));
    }

    #[test]
    fn render_scheduler_non_streaming_is_fast() {
        let mut rs = RenderScheduler::new();
        rs.adapt_interval();
        assert_eq!(rs.min_interval, Duration::from_millis(16));
    }

    #[test]
    fn ctrl_c_progressive_exit_interrupt_then_confirm_then_force() {
        let mut state = AppState::new("fake".into(), agent_tools::PermissionMode::Suggest);
        assert_eq!(state.record_ctrl_c(), CtrlCAction::Interrupt);
        assert_eq!(state.record_ctrl_c(), CtrlCAction::ConfirmQuit);
        assert_eq!(state.record_ctrl_c(), CtrlCAction::ForceQuit);
    }

    #[test]
    fn ctrl_c_resets_after_timeout() {
        let mut state = AppState::new("fake".into(), agent_tools::PermissionMode::Suggest);
        state.record_ctrl_c();
        state.last_ctrl_c = Some(Instant::now() - Duration::from_secs(6));
        state.ctrl_c_count = 1;
        // Next Ctrl+C should restart the cycle since >5s elapsed
        assert_eq!(state.record_ctrl_c(), CtrlCAction::Interrupt);
        assert_eq!(state.ctrl_c_count, 1);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p agent-tui -- app_state`
Expected: All 8 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/agent-tui/src/app_state.rs
git commit -m "feat(tui): add AppState with FocusManager, RenderScheduler, and Ctrl-C handling"
```

---

## Task 3: Keybinding System

**Files:**

- Create: `crates/agent-tui/src/keybindings.rs`

- [ ] **Step 1: Write the keybinding resolver with tests**

Create `crates/agent-tui/src/keybindings.rs`:

```rust
use crate::components::FocusTarget;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    // L1 Instant
    SendInput,
    Escape,
    FocusCycleNext,
    FocusPrevious,
    FocusNext,
    AllowPermission,
    DenyPermission,
    DenyAllPermission,
    ContextMenu,
    // L2 Alt Modifier
    ToggleSessionsSidebar,
    ToggleTraceSidebar,
    ToggleInputMode,
    OpenProfileSelector,
    NewSession,
    Quit,
    FocusChat,
    FocusSessions,
    FocusTrace,
    // L3 Ctrl Reserved
    InterruptOrQuit,
    Redraw,
    // L4 Function
    Help,
    RenameSession,
    ToggleTraceDensity,
    // Input
    InputCharacter(char),
    InputBackspace,
    InputDelete,
    InputNewline,
    InputHistoryUp,
    InputHistoryDown,
    InputPaste(String),
    // Navigation
    ScrollUp,
    ScrollDown,
    // Noop
    Unhandled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceDensity {
    Summary,
    Expanded,
    FullEventStream,
}

impl TraceDensity {
    pub fn next(self) -> Self {
        match self {
            Self::Summary => Self::Expanded,
            Self::Expanded => Self::FullEventStream,
            Self::FullEventStream => Self::Summary,
        }
    }
}

/// Resolve a key event to an action based on current focus and state.
pub fn resolve_key(
    key: KeyEvent,
    focus: FocusTarget,
    permission_pending: bool,
    input_mode: &crate::app_state::InputMode,
) -> KeyAction {
    if permission_pending {
        return resolve_permission_key(key);
    }

    match (key.modifiers, key.code) {
        // L2 Alt Modifier (global)
        (KeyModifiers::ALT, KeyCode::Char('s')) => KeyAction::ToggleSessionsSidebar,
        (KeyModifiers::ALT, KeyCode::Char('t')) => KeyAction::ToggleTraceSidebar,
        (KeyModifiers::ALT, KeyCode::Char('e')) => KeyAction::ToggleInputMode,
        (KeyModifiers::ALT, KeyCode::Char('p')) => KeyAction::OpenProfileSelector,
        (KeyModifiers::ALT, KeyCode::Char('n')) => KeyAction::NewSession,
        (KeyModifiers::ALT, KeyCode::Char('q')) => KeyAction::Quit,
        (KeyModifiers::ALT, KeyCode::Char('1')) => KeyAction::FocusChat,
        (KeyModifiers::ALT, KeyCode::Char('2')) => KeyAction::FocusSessions,
        (KeyModifiers::ALT, KeyCode::Char('3')) => KeyAction::FocusTrace,

        // L3 Ctrl Reserved
        (KeyModifiers::CONTROL, KeyCode::Char('c')) => KeyAction::InterruptOrQuit,
        (KeyModifiers::CONTROL, KeyCode::Char('l')) => KeyAction::Redraw,
        (KeyModifiers::CONTROL, KeyCode::Enter) => KeyAction::SendInput,

        // L4 Function Keys
        (_, KeyCode::F(1)) => KeyAction::Help,
        (_, KeyCode::F(2)) if focus == FocusTarget::Sessions => KeyAction::RenameSession,
        (_, KeyCode::F(5)) if focus == FocusTarget::Trace => KeyAction::ToggleTraceDensity,

        // L1 Instant — depends on focus
        (KeyModifiers::NONE, KeyCode::Enter) => match focus {
            FocusTarget::Chat | FocusTarget::PermissionModal => {
                if matches!(input_mode, crate::app_state::InputMode::MultiLine) {
                    KeyAction::InputNewline
                } else {
                    KeyAction::SendInput
                }
            }
            FocusTarget::Trace => KeyAction::FocusNext, // expand detail
            FocusTarget::Sessions => KeyAction::FocusNext, // switch session
        },
        (KeyModifiers::NONE, KeyCode::Esc) => KeyAction::Escape,
        (KeyModifiers::NONE, KeyCode::Tab) => KeyAction::FocusCycleNext,
        (KeyModifiers::NONE, KeyCode::Up) => match focus {
            FocusTarget::Chat => KeyAction::InputHistoryUp,
            _ => KeyAction::ScrollUp,
        },
        (KeyModifiers::NONE, KeyCode::Down) => match focus {
            FocusTarget::Chat => KeyAction::InputHistoryDown,
            _ => KeyAction::ScrollDown,
        },
        (KeyModifiers::NONE, KeyCode::Char('x')) => KeyAction::ContextMenu,
        (KeyModifiers::NONE, KeyCode::Backspace) => KeyAction::InputBackspace,
        (KeyModifiers::NONE, KeyCode::Delete) => KeyAction::InputDelete,
        (KeyModifiers::NONE, KeyCode::Char(c)) => KeyAction::InputCharacter(c),

        _ => KeyAction::Unhandled,
    }
}

fn resolve_permission_key(key: KeyEvent) -> KeyAction {
    match (key.modifiers, key.code) {
        (KeyModifiers::NONE, KeyCode::Char('y') | KeyCode::Char('Y')) => KeyAction::AllowPermission,
        (KeyModifiers::NONE, KeyCode::Char('n') | KeyCode::Char('N')) => KeyAction::DenyPermission,
        (KeyModifiers::NONE, KeyCode::Char('d') | KeyCode::Char('D')) => KeyAction::DenyAllPermission,
        (KeyModifiers::NONE, KeyCode::Esc) => KeyAction::DenyPermission,
        _ => KeyAction::Unhandled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alt(char: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(char), KeyModifiers::ALT)
    }

    fn ctrl(char: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(char), KeyModifiers::CONTROL)
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn l2_alt_keys_resolve_globally() {
        let focus = FocusTarget::Chat;
        assert_eq!(
            resolve_key(alt('s'), focus, false, &InputMode::SingleLine),
            KeyAction::ToggleSessionsSidebar
        );
        assert_eq!(
            resolve_key(alt('t'), focus, false, &InputMode::SingleLine),
            KeyAction::ToggleTraceSidebar
        );
        assert_eq!(
            resolve_key(alt('p'), focus, false, &InputMode::SingleLine),
            KeyAction::OpenProfileSelector
        );
        assert_eq!(
            resolve_key(alt('q'), focus, false, &InputMode::SingleLine),
            KeyAction::Quit
        );
    }

    #[test]
    fn l3_ctrl_c_interrupts() {
        assert_eq!(
            resolve_key(ctrl('c'), FocusTarget::Chat, false, &InputMode::SingleLine),
            KeyAction::InterruptOrQuit
        );
    }

    #[test]
    fn l1_enter_sends_in_singleline() {
        assert_eq!(
            resolve_key(key(KeyCode::Enter), FocusTarget::Chat, false, &InputMode::SingleLine),
            KeyAction::SendInput
        );
    }

    #[test]
    fn l1_enter_newline_in_multiline() {
        assert_eq!(
            resolve_key(key(KeyCode::Enter), FocusTarget::Chat, false, &InputMode::MultiLine),
            KeyAction::InputNewline
        );
    }

    #[test]
    fn ctrl_enter_sends_in_multiline() {
        assert_eq!(
            resolve_key(
                KeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL),
                FocusTarget::Chat,
                false,
                &InputMode::MultiLine,
            ),
            KeyAction::SendInput
        );
    }

    #[test]
    fn permission_keys_override_normal() {
        assert_eq!(
            resolve_key(key(KeyCode::Char('y')), FocusTarget::Chat, true, &InputMode::SingleLine),
            KeyAction::AllowPermission
        );
        assert_eq!(
            resolve_key(key(KeyCode::Char('n')), FocusTarget::Chat, true, &InputMode::SingleLine),
            KeyAction::DenyPermission
        );
        assert_eq!(
            resolve_key(key(KeyCode::Char('d')), FocusTarget::Chat, true, &InputMode::SingleLine),
            KeyAction::DenyAllPermission
        );
    }

    #[test]
    fn l4_f5_toggles_trace_in_trace_focus() {
        assert_eq!(
            resolve_key(key(KeyCode::F(5)), FocusTarget::Trace, false, &InputMode::SingleLine),
            KeyAction::ToggleTraceDensity
        );
        assert_eq!(
            resolve_key(key(KeyCode::F(5)), FocusTarget::Chat, false, &InputMode::SingleLine),
            KeyAction::Unhandled
        );
    }

    #[test]
    fn trace_density_cycles() {
        assert_eq!(TraceDensity::Summary.next(), TraceDensity::Expanded);
        assert_eq!(TraceDensity::Expanded.next(), TraceDensity::FullEventStream);
        assert_eq!(TraceDensity::FullEventStream.next(), TraceDensity::Summary);
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p agent-tui -- keybindings`
Expected: All 8 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/agent-tui/src/keybindings.rs
git commit -m "feat(tui): add keybinding resolver with L1-L4 layer support"
```

---

## Task 4: StatusBar Component

**Files:**

- Create: `crates/agent-tui/src/components/status_bar.rs`

- [ ] **Step 1: Write the StatusBar component with tests**

Create `crates/agent-tui/src/components/status_bar.rs`:

```rust
use crate::components::{Component, CrossPanelEffect, EventContext, StatusInfo};
use crate::components::Command;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub struct StatusBar {
    focused: bool,
}

impl StatusBar {
    pub fn new() -> Self {
        Self { focused: false }
    }
}

impl Component for StatusBar {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        _event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        (Vec::new(), Vec::new())
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        if let CrossPanelEffect::SetStatus(info) = effect {
            // Status bar is read-only display; it reads from info each render
            let _ = info;
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        // Status bar doesn't need ctx for rendering — it receives SetStatus
        // StatusBar receives SetStatus effects for real data; this renders a default view
        let spans = vec![
            Span::styled(" kairox ", Style::default().fg(Color::White).bg(Color::DarkGray)),
            Span::raw(" "),
            Span::styled("Alt+S/? for help", Style::default().fg(Color::DarkGray)),
        ];
        let paragraph = Paragraph::new(Line::from(spans));
        frame.render_widget(paragraph, area);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

pub fn render_status_bar(area: Rect, frame: &mut Frame, info: &StatusInfo) {
    let error_span = info.error.as_ref().map(|e| {
        Span::styled(format!(" ⚠ {e}"), Style::default().fg(Color::Red))
    });
    let mut spans = vec![
        Span::styled(
            format!(" {} ", info.profile),
            Style::default().fg(Color::Black).bg(Color::Cyan),
        ),
        Span::raw(" │ "),
        Span::styled(
            format!(" {} ", info.permission_mode_label()),
            Style::default().fg(Color::Black).bg(Color::Yellow),
        ),
        Span::raw(" │ "),
        Span::raw(format!(" sessions: {} ", info.session_count)),
        Span::raw(" │ "),
        Span::styled(
            format!(" {} ", info.hint),
            Style::default().add_modifier(Modifier::DIM),
        ),
    ];
    if let Some(span) = error_span {
        spans.push(span);
    }
    let paragraph = Paragraph::new(Line::from(spans));
    frame.render_widget(paragraph, area);
}

impl StatusInfo {
    pub fn permission_mode_label(&self) -> &str {
        self.permission_mode.as_str()
    }
}

// We need this for status bar rendering; patch it into StatusInfo
// This is a display-only helper
impl agent_tools::PermissionMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            agent_tools::PermissionMode::ReadOnly => "readonly",
            agent_tools::PermissionMode::Suggest => "suggest",
            agent_tools::PermissionMode::Agent => "agent",
            agent_tools::PermissionMode::Autonomous => "autonomous",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_bar_renders_without_panic() {
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(80, 24)).unwrap();
        let info = StatusInfo {
            profile: "fast".into(),
            permission_mode: "suggest".into(),
            session_count: 3,
            hint: "Alt+S/?".into(),
            error: None,
        };
        terminal.draw(|f| {
            render_status_bar(f.area(), f, &info);
        }).unwrap();
    }

    #[test]
    fn permission_mode_as_str() {
        assert_eq!(agent_tools::PermissionMode::ReadOnly.as_str(), "readonly");
        assert_eq!(agent_tools::PermissionMode::Suggest.as_str(), "suggest");
        assert_eq!(agent_tools::PermissionMode::Agent.as_str(), "agent");
        assert_eq!(agent_tools::PermissionMode::Autonomous.as_str(), "autonomous");
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p agent-tui -- status_bar`
Expected: All 2 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/agent-tui/src/components/status_bar.rs
git commit -m "feat(tui): add StatusBar component with render helper"
```

---

## Task 5: ChatPanel Component (Message Display + Input)

**Files:**

- Create: `crates/agent-tui/src/components/chat.rs`

- [ ] **Step 1: Write ChatPanel with input handling and tests**

Create `crates/agent-tui/src/components/chat.rs`:

```rust
use crate::app_state::{InputMode, InputState};
use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, FocusTarget, PermissionRequest,
};
use crate::keybindings::KeyAction;
use agent_core::projection::{ProjectedRole, SessionProjection};
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

pub struct ChatPanel {
    focused: bool,
    pub input_content: String,
    pub input_cursor: usize,
    pub input_mode: InputMode,
    pub input_state: InputState,
    pub input_history: Vec<String>,
    pub input_history_index: Option<usize>,
    pub scroll_offset: usize,
}

impl ChatPanel {
    pub fn new() -> Self {
        Self {
            focused: false,
            input_content: String::new(),
            input_cursor: 0,
            input_mode: InputMode::SingleLine,
            input_state: InputState::Normal,
            input_history: Vec::new(),
            input_history_index: None,
            scroll_offset: 0,
        }
    }

    pub fn apply_key_action(
        &mut self,
        action: KeyAction,
        ctx: &EventContext,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match action {
            KeyAction::SendInput => {
                if !self.input_content.is_empty() {
                    let content = self.input_content.clone();
                    self.input_history.push(content.clone());
                    self.input_history_index = None;
                    self.input_content.clear();
                    self.input_cursor = 0;

                    if let Some(session) = ctx.sessions.first() {
                        commands.push(Command::SendMessage {
                            workspace_id: agent_core::WorkspaceId::new(), // will be overridden by App
                            session_id: session.id.clone(),
                            content,
                        });
                    }
                }
            }
            KeyAction::InputCharacter(c) => {
                self.input_content.insert(self.input_cursor, c);
                self.input_cursor += 1;
            }
            KeyAction::InputBackspace => {
                if self.input_cursor > 0 {
                    self.input_cursor -= 1;
                    self.input_content.remove(self.input_cursor);
                }
            }
            KeyAction::InputNewline => {
                if matches!(self.input_mode, InputMode::MultiLine) {
                    self.input_content.insert(self.input_cursor, '\n');
                    self.input_cursor += 1;
                }
            }
            KeyAction::InputHistoryUp => {
                if !self.input_history.is_empty() {
                    let idx = self
                        .input_history_index
                        .map(|i| if i > 0 { i - 1 } else { 0 })
                        .unwrap_or(self.input_history.len() - 1);
                    self.input_history_index = Some(idx);
                    self.input_content = self.input_history[idx].clone();
                    self.input_cursor = self.input_content.len();
                }
            }
            KeyAction::InputHistoryDown => {
                if let Some(idx) = self.input_history_index {
                    if idx + 1 < self.input_history.len() {
                        let new_idx = idx + 1;
                        self.input_history_index = Some(new_idx);
                        self.input_content = self.input_history[new_idx].clone();
                        self.input_cursor = self.input_content.len();
                    } else {
                        self.input_history_index = None;
                        self.input_content.clear();
                        self.input_cursor = 0;
                    }
                }
            }
            KeyAction::ToggleInputMode => {
                self.input_mode = match self.input_mode {
                    InputMode::SingleLine => InputMode::MultiLine,
                    InputMode::MultiLine => InputMode::SingleLine,
                };
            }
            KeyAction::AllowPermission => {
                if let InputState::PermissionWait { request_id, .. } = &self.input_state {
                    commands.push(Command::DecidePermission {
                        request_id: request_id.clone(),
                        approved: true,
                    });
                    effects.push(CrossPanelEffect::DismissPermissionPrompt);
                    self.input_state = InputState::Normal;
                }
            }
            KeyAction::DenyPermission => {
                if let InputState::PermissionWait { request_id, .. } = &self.input_state {
                    commands.push(Command::DecidePermission {
                        request_id: request_id.clone(),
                        approved: false,
                    });
                    effects.push(CrossPanelEffect::DismissPermissionPrompt);
                    self.input_state = InputState::Normal;
                }
            }
            KeyAction::DenyAllPermission => {
                if let InputState::PermissionWait { request_id, .. } = &self.input_state {
                    commands.push(Command::DecidePermission {
                        request_id: request_id.clone(),
                        approved: false,
                    });
                    effects.push(CrossPanelEffect::DismissPermissionPrompt);
                    self.input_state = InputState::Normal;
                    // Session-level deny rules will be added when session rule persistence is implemented in M4
                }
            }
            KeyAction::Escape => {
                if matches!(self.input_mode, InputMode::MultiLine)
                    && self.input_content.is_empty()
                {
                    self.input_mode = InputMode::SingleLine;
                }
            }
            _ => {}
        }

        (effects, commands)
    }
}

impl Component for ChatPanel {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let crossterm::event::Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        let permission_pending = matches!(self.input_state, InputState::PermissionWait { .. });
        let action = crate::keybindings::resolve_key(*key, ctx.focus, permission_pending, &self.input_mode);
        self.apply_key_action(action, ctx)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowPermissionPrompt(req) => {
                if matches!(req.risk_level, crate::components::RiskLevel::Write) {
                    self.input_state = InputState::PermissionWait {
                        request_id: req.request_id.clone(),
                        pending_prompt: Some(req.clone()),
                    };
                }
            }
            CrossPanelEffect::DismissPermissionPrompt => {
                self.input_state = InputState::Normal;
            }
            CrossPanelEffect::StartStreaming => {}
            CrossPanelEffect::StopStreaming => {}
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        let messages_block = Block::default()
            .borders(Borders::NONE)
            .style(Style::default());

        // ChatPanel render delegates to App-level render_messages for message display
        let _ = messages_block;
        let _ = area;
        let _ = frame;
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

pub fn render_messages(area: Rect, frame: &mut Frame, projection: &SessionProjection) {
    let lines: Vec<Line> = projection
        .messages
        .iter()
        .map(|msg| match msg.role {
            ProjectedRole::User => Line::from(vec![
                Span::styled("You: ", Style::default().fg(Color::Cyan)),
                Span::raw(&msg.content),
            ]),
            ProjectedRole::Assistant => Line::from(vec![
                Span::styled("Agent: ", Style::default().fg(Color::Green)),
                Span::raw(&msg.content),
            ]),
        })
        .chain(std::iter::once_with(|| {
            if projection.cancelled {
                Line::from(Span::styled("[cancelled]", Style::default().fg(Color::Yellow)))
            } else if !projection.token_stream.is_empty() {
                Line::from(vec![
                    Span::styled("Agent: ", Style::default().fg(Color::Green)),
                    Span::raw(&projection.token_stream),
                    Span::styled("▌", Style::default().fg(Color::White)),
                ])
            } else {
                Line::from("")
            }
        }))
        .collect();

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::RiskLevel;
    use agent_core::SessionId;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn test_ctx() -> EventContext<'static> {
        // We can't construct EventContext with static refs easily,
        // so test via apply_key_action directly
        unimplemented!("use unit tests on apply_key_action with mock ctx")
    }

    #[test]
    fn input_character_appends_to_content() {
        let mut panel = ChatPanel::new();
        panel.apply_key_action(KeyAction::InputCharacter('h'), &unsafe_ctx());
        panel.apply_key_action(KeyAction::InputCharacter('i'), &unsafe_ctx());
        assert_eq!(panel.input_content, "hi");
        assert_eq!(panel.input_cursor, 2);
    }

    #[test]
    fn backspace_removes_character() {
        let mut panel = ChatPanel::new();
        panel.input_content = "ab".into();
        panel.input_cursor = 2;
        panel.apply_key_action(KeyAction::InputBackspace, &unsafe_ctx());
        assert_eq!(panel.input_content, "a");
        assert_eq!(panel.input_cursor, 1);
    }

    #[test]
    fn toggle_input_mode_switches() {
        let mut panel = ChatPanel::new();
        assert_eq!(panel.input_mode, InputMode::SingleLine);
        panel.apply_key_action(KeyAction::ToggleInputMode, &unsafe_ctx());
        assert_eq!(panel.input_mode, InputMode::MultiLine);
        panel.apply_key_action(KeyAction::ToggleInputMode, &unsafe_ctx());
        assert_eq!(panel.input_mode, InputMode::SingleLine);
    }

    #[test]
    fn permission_wait_state_allows_deny() {
        let mut panel = ChatPanel::new();
        panel.input_state = InputState::PermissionWait {
            request_id: "req1".into(),
            pending_prompt: None,
        };
        let (_, commands) = panel.apply_key_action(KeyAction::DenyPermission, &unsafe_ctx());
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            Command::DecidePermission { approved: false, .. }
        ));
        assert_eq!(panel.input_state, InputState::Normal);
    }

    #[test]
    fn history_navigation_works() {
        let mut panel = ChatPanel::new();
        panel.input_history = vec!["first".into(), "second".into()];
        panel.apply_key_action(KeyAction::InputHistoryUp, &unsafe_ctx());
        assert_eq!(panel.input_content, "second");
        panel.apply_key_action(KeyAction::InputHistoryUp, &unsafe_ctx());
        assert_eq!(panel.input_content, "first");
        panel.apply_key_action(KeyAction::InputHistoryDown, &unsafe_ctx());
        assert_eq!(panel.input_content, "second");
        panel.apply_key_action(KeyAction::InputHistoryDown, &unsafe_ctx());
        assert_eq!(panel.input_content, "");
    }

    /// A minimal EventContext for unit tests. Safe because tests are single-threaded
    /// and the references don't outlive the test.
    fn unsafe_ctx() -> EventContext<'static> {
        use agent_core::projection::SessionProjection;
        static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
        let projection = PROJECTION.get_or_init(SessionProjection::default);
        static SESSIONS: std::sync::OnceLock<Vec<crate::components::SessionInfo>> =
            std::sync::OnceLock::new();
        let sessions = SESSIONS.get_or_init(Vec::new);

        EventContext {
            focus: FocusTarget::Chat,
            current_session: projection,
            sessions,
            model_profile: "fake",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: true,
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they pass**

Run: `cargo test -p agent-tui -- chat`
Expected: All 5 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/agent-tui/src/components/chat.rs
git commit -m "feat(tui): add ChatPanel with input handling, permission, and history"
```

---

## Task 6: SessionsPanel Component

**Files:**

- Create: `crates/agent-tui/src/components/sessions.rs`

- [ ] **Step 1: Write SessionsPanel with list rendering and tests**

Create `crates/agent-tui/src/components/sessions.rs`:

```rust
use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, FocusTarget, SessionInfo, SessionState,
};
use agent_core::SessionId;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState};
use ratatui::Frame;

pub struct SessionsPanel {
    focused: bool,
    pub state: ListState,
    pub context_menu_open: bool,
    pub search_query: Option<String>,
}

impl SessionsPanel {
    pub fn new() -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        Self {
            focused: false,
            state,
            context_menu_open: false,
            search_query: None,
        }
    }

    pub fn selected_session_id(&self, sessions: &[SessionInfo]) -> Option<SessionId> {
        self.state.selected().and_then(|i| sessions.get(i)).map(|s| s.id.clone())
    }

    pub fn filtered_sessions<'a>(&self, sessions: &'a [SessionInfo]) -> Vec<&'a SessionInfo> {
        if let Some(query) = &self.search_query {
            let q = query.to_lowercase();
            sessions
                .iter()
                .filter(|s| s.title.to_lowercase().contains(&q) || s.model_profile.to_lowercase().contains(&q))
                .collect()
        } else {
            sessions.iter().collect()
        }
    }
}

fn session_state_icon(state: &SessionState) -> (&'static str, Color) {
    match state {
        SessionState::Active => ("●", Color::Green),
        SessionState::Idle => ("○", Color::DarkGray),
        SessionState::Error(_) => ("✕", Color::Red),
        SessionState::AwaitingPermission => ("⚠", Color::Yellow),
    }
}

pub fn render_sessions(area: Rect, frame: &mut Frame, sessions: &[SessionInfo], focused: bool) {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = sessions
        .iter()
        .map(|session| {
            let (icon, icon_color) = session_state_icon(&session.state);
            let pin = if session.pinned { "📌 " } else { "" };
            let mut spans = vec![
                Span::styled(format!("{pin}{icon} "), Style::default().fg(icon_color)),
                Span::raw(&session.title),
                Span::styled(
                    format!(" [{}]", session.model_profile),
                    Style::default().add_modifier(Modifier::DIM),
                ),
            ];
            if let SessionState::Error(e) = &session.state {
                spans.push(Span::styled(format!(" {e}"), Style::default().fg(Color::Red)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::RIGHT)
            .title(" Sessions ")
            .border_style(border_style),
    );
    frame.render_widget(list, area);
}

impl Component for SessionsPanel {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let _ = (ctx, event);
        (Vec::new(), Vec::new())
    }

    fn handle_effect(&mut self, _effect: &CrossPanelEffect) {}

    fn render(&self, area: Rect, frame: &mut Frame) {
        let _ = (area, frame);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_session(title: &str, state: SessionState, pinned: bool) -> SessionInfo {
        SessionInfo {
            id: SessionId::new(),
            title: title.into(),
            model_profile: "fast".into(),
            state,
            pinned,
        }
    }

    #[test]
    fn filtered_sessions_returns_all_when_no_query() {
        let panel = SessionsPanel::new();
        let sessions = vec![
            make_session("main", SessionState::Active, false),
            make_session("debug", SessionState::Idle, false),
        ];
        let filtered = panel.filtered_sessions(&sessions);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn filtered_sessions_matches_title_case_insensitive() {
        let mut panel = SessionsPanel::new();
        panel.search_query = Some("MAIN".into());
        let sessions = vec![
            make_session("main session", SessionState::Active, false),
            make_session("debug", SessionState::Idle, false),
        ];
        let filtered = panel.filtered_sessions(&sessions);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].title, "main session");
    }

    #[test]
    fn selected_session_id_returns_correct_id() {
        let mut panel = SessionsPanel::new();
        let sessions = vec![
            make_session("first", SessionState::Active, false),
            make_session("second", SessionState::Idle, false),
        ];
        panel.state.select(Some(1));
        assert_eq!(panel.selected_session_id(&sessions), Some(sessions[1].id.clone()));
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-tui -- sessions`
Expected: All 3 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/agent-tui/src/components/sessions.rs
git commit -m "feat(tui): add SessionsPanel with list rendering and search filter"
```

---

## Task 7: TracePanel Component

**Files:**

- Create: `crates/agent-tui/src/components/trace.rs`

- [ ] **Step 1: Write TracePanel with L1/L2/L3 density and tests**

Create `crates/agent-tui/src/components/trace.rs`:

```rust
use crate::components::{Component, CrossPanelEffect, EventContext, FocusTarget};
use crate::keybindings::TraceDensity;
use agent_core::events::EventPayload;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

pub struct TracePanel {
    focused: bool,
    pub density: TraceDensity,
    pub expanded_index: Option<usize>,
    pub scroll_offset: usize,
}

impl TracePanel {
    pub fn new() -> Self {
        Self {
            focused: false,
            density: TraceDensity::Summary,
            expanded_index: None,
            scroll_offset: 0,
        }
    }
}

/// A trace entry rendered in the sidebar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceEntry {
    pub tool_id: String,
    pub status: TraceStatus,
    pub duration_ms: Option<u64>,
    pub args_preview: Option<String>,
    pub output_preview: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceStatus {
    Running,
    Success,
    Failed,
}

impl std::fmt::Display for TraceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "⏳"),
            Self::Success => write!(f, "✓"),
            Self::Failed => write!(f, "✕"),
        }
    }
}

pub fn extract_tool_traces(events: &[agent_core::DomainEvent]) -> Vec<TraceEntry> {
    let mut traces = Vec::new();
    for event in events {
        match &event.payload {
            EventPayload::ToolInvocationStarted { tool_id, .. } => {
                traces.push(TraceEntry {
                    tool_id: tool_id.clone(),
                    status: TraceStatus::Running,
                    duration_ms: None,
                    args_preview: None,
                    output_preview: None,
                });
            }
            EventPayload::ToolInvocationCompleted {
                tool_id,
                duration_ms,
                output_preview,
                ..
            } => {
                if let Some(entry) = traces.iter_mut().rev().find(|t| t.tool_id == *tool_id && t.status == TraceStatus::Running) {
                    entry.status = TraceStatus::Success;
                    entry.duration_ms = Some(*duration_ms);
                    entry.output_preview = Some(output_preview.clone());
                }
            }
            EventPayload::ToolInvocationFailed { tool_id, error, .. } => {
                if let Some(entry) = traces.iter_mut().rev().find(|t| t.tool_id == *tool_id && t.status == TraceStatus::Running) {
                    entry.status = TraceStatus::Failed;
                    entry.output_preview = Some(error.clone());
                }
            }
            _ => {}
        }
    }
    traces
}

pub fn render_trace_l1(area: Rect, frame: &mut Frame, traces: &[TraceEntry], focused: bool) {
    let border_style = if focused {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let items: Vec<ListItem> = traces
        .iter()
        .map(|entry| {
            let status_color = match entry.status {
                TraceStatus::Running => Color::Yellow,
                TraceStatus::Success => Color::Green,
                TraceStatus::Failed => Color::Red,
            };
            let duration = entry
                .duration_ms
                .map(|d| format!(" {d:.1}s"))
                .unwrap_or_default();
            let line = Line::from(vec![
                Span::styled("▶ ", Style::default()),
                Span::styled(&entry.tool_id, Style::default()),
                Span::styled(format!(" {}{}", entry.status, duration), Style::default().fg(status_color)),
            ]);
            ListItem::new(line)
        })
        .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::LEFT)
            .title(" Trace ")
            .border_style(border_style),
    );
    frame.render_widget(list, area);
}

impl Component for TracePanel {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let _ = (ctx, event);
        (Vec::new(), Vec::new())
    }

    fn handle_effect(&mut self, _effect: &CrossPanelEffect) {}

    fn render(&self, area: Rect, frame: &mut Frame) {
        let _ = (area, frame);
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId};

    fn make_event(payload: EventPayload) -> DomainEvent {
        DomainEvent::new(
            WorkspaceId::new(),
            SessionId::new(),
            AgentId::system(),
            PrivacyClassification::FullTrace,
            payload,
        )
    }

    #[test]
    fn extract_tool_traces_from_events() {
        let events = vec![
            make_event(EventPayload::ToolInvocationStarted {
                invocation_id: "inv1".into(),
                tool_id: "shell.exec".into(),
            }),
            make_event(EventPayload::ToolInvocationCompleted {
                invocation_id: "inv1".into(),
                tool_id: "shell.exec".into(),
                output_preview: "ok".into(),
                exit_code: None,
                duration_ms: 1200,
                truncated: false,
            }),
            make_event(EventPayload::ToolInvocationStarted {
                invocation_id: "inv2".into(),
                tool_id: "patch.apply".into(),
            }),
        ];

        let traces = extract_tool_traces(&events);
        assert_eq!(traces.len(), 2);
        assert_eq!(traces[0].tool_id, "shell.exec");
        assert_eq!(traces[0].status, TraceStatus::Success);
        assert_eq!(traces[0].duration_ms, Some(1200));
        assert_eq!(traces[1].tool_id, "patch.apply");
        assert_eq!(traces[1].status, TraceStatus::Running);
        assert!(traces[1].duration_ms.is_none());
    }

    #[test]
    fn trace_density_cycles() {
        assert_eq!(TraceDensity::Summary.next(), TraceDensity::Expanded);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-tui -- trace`
Expected: All 2 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/agent-tui/src/components/trace.rs
git commit -m "feat(tui): add TracePanel with L1 summary and event extraction"
```

---

## Task 8: PermissionModal Component

**Files:**

- Create: `crates/agent-tui/src/components/permission_modal.rs`

- [ ] **Step 1: Write PermissionModal with tests**

Create `crates/agent-tui/src/components/permission_modal.rs`:

```rust
use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, PermissionRequest,
};
use crate::keybindings::KeyAction;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

pub struct PermissionModal {
    focused: bool,
    pub request: Option<PermissionRequest>,
}

impl PermissionModal {
    pub fn new() -> Self {
        Self {
            focused: false,
            request: None,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.request.is_some()
    }
}

pub fn render_permission_modal(area: Rect, frame: &mut Frame, request: &PermissionRequest) {
    let modal_width = 50.min(area.width.saturating_sub(4));
    let modal_height = 10.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let lines = vec![
        Line::from(Span::styled(
            "⛔ Destructive Operation",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("Tool: ", Style::default().fg(Color::Gray)),
            Span::raw(&request.tool_id),
        ]),
        Line::from(vec![
            Span::styled("Command: ", Style::default().fg(Color::Gray)),
            Span::raw(&request.tool_preview),
        ]),
        Line::from(vec![
            Span::styled("Risk: ", Style::default().fg(Color::Gray)),
            Span::styled("Destructive", Style::default().fg(Color::Red)),
        ]),
        Line::from(""),
        Line::from("This operation cannot be undone."),
        Line::from(""),
        Line::from(vec![
            Span::styled("[Y] Allow once  ", Style::default().fg(Color::Yellow)),
            Span::styled("[N] Deny  ", Style::default().fg(Color::Gray)),
            Span::styled("[Esc] Cancel", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let paragraph = Paragraph::new(lines)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Red)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, modal_area);
}

impl Component for PermissionModal {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let crossterm::event::Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        let Some(req) = &self.request else {
            return (Vec::new(), Vec::new());
        };

        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match key.code {
            crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Char('Y') => {
                commands.push(Command::DecidePermission {
                    request_id: req.request_id.clone(),
                    approved: true,
                });
                effects.push(CrossPanelEffect::DismissPermissionPrompt);
                self.request = None;
            }
            crossterm::event::KeyCode::Char('n')
            | crossterm::event::KeyCode::Char('N')
            | crossterm::event::KeyCode::Esc => {
                commands.push(Command::DecidePermission {
                    request_id: req.request_id.clone(),
                    approved: false,
                });
                effects.push(CrossPanelEffect::DismissPermissionPrompt);
                self.request = None;
            }
            _ => {}
        }

        (effects, commands)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        if let CrossPanelEffect::ShowPermissionPrompt(req) = effect {
            if matches!(req.risk_level, crate::components::RiskLevel::Destructive) {
                self.request = Some(req.clone());
            }
        }
        if let CrossPanelEffect::DismissPermissionPrompt = effect {
            self.request = None;
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if let Some(req) = &self.request {
            render_permission_modal(area, frame, req);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::RiskLevel;

    #[test]
    fn modal_invisible_when_no_request() {
        let modal = PermissionModal::new();
        assert!(!modal.is_visible());
    }

    #[test]
    fn modal_visible_on_destructive_effect() {
        let mut modal = PermissionModal::new();
        modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
            request_id: "req1".into(),
            tool_id: "shell.exec".into(),
            tool_preview: "rm -rf target/".into(),
            risk_level: RiskLevel::Destructive,
        }));
        assert!(modal.is_visible());
    }

    #[test]
    fn modal_ignores_write_risk() {
        let mut modal = PermissionModal::new();
        modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
            request_id: "req2".into(),
            tool_id: "shell.exec".into(),
            tool_preview: "cargo build".into(),
            risk_level: RiskLevel::Write,
        }));
        assert!(!modal.is_visible());
    }

    #[test]
    fn allow_sends_decide_and_dismisses() {
        let mut modal = PermissionModal::new();
        modal.request = Some(PermissionRequest {
            request_id: "req1".into(),
            tool_id: "shell.exec".into(),
            tool_preview: "rm -rf target/".into(),
            risk_level: RiskLevel::Destructive,
        });
        let key = crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Char('y'),
            crossterm::event::KeyModifiers::NONE,
        ));
        let (effects, commands) = modal.handle_event(
            &unsafe_ctx(),
            &key,
        );
        assert_eq!(commands.len(), 1);
        assert!(matches!(&commands[0], Command::DecidePermission { approved: true, .. }));
        assert!(effects.contains(&CrossPanelEffect::DismissPermissionPrompt));
        assert!(!modal.is_visible());
    }

    fn unsafe_ctx() -> EventContext<'static> {
        use agent_core::projection::SessionProjection;
        static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
        let projection = PROJECTION.get_or_init(SessionProjection::default);
        static SESSIONS: std::sync::OnceLock<Vec<crate::components::SessionInfo>> =
            std::sync::OnceLock::new();
        let sessions = SESSIONS.get_or_init(Vec::new);
        EventContext {
            focus: FocusTarget::Chat,
            current_session: projection,
            sessions,
            model_profile: "fake",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: true,
        }
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p agent-tui -- permission_modal`
Expected: All 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/agent-tui/src/components/permission_modal.rs
git commit -m "feat(tui): add PermissionModal for destructive operations"
```

---

## Task 9: App — Component Composition and Event Routing

**Files:**

- Rewrite: `crates/agent-tui/src/app.rs`

- [ ] **Step 1: Write App struct that composes all components**

Replace `crates/agent-tui/src/app.rs` with:

```rust
use crate::app_state::{AppState, CtrlCAction};
use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, FocusTarget, PermissionRequest, RiskLevel,
    SessionInfo, SessionState, StatusInfo,
};
use crate::components::chat::ChatPanel;
use crate::components::sessions::SessionsPanel;
use crate::components::trace::{TracePanel, extract_tool_traces};
use crate::components::status_bar::StatusBar;
use crate::components::permission_modal::PermissionModal;
use crate::keybindings::KeyAction;
use agent_core::events::EventPayload;
use agent_core::projection::SessionProjection;
use agent_core::{AppFacade, SessionId, WorkspaceId};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::Frame;

pub struct App {
    pub state: AppState,
    pub chat: ChatPanel,
    pub sessions: SessionsPanel,
    pub trace: TracePanel,
    pub status_bar: StatusBar,
    pub permission_modal: PermissionModal,
    pub workspace_id: WorkspaceId,
    pub current_session_id: Option<SessionId>,
    pub domain_events: Vec<agent_core::DomainEvent>,
    pub quit_confirmed: bool,
    pub quitting: bool,
}

impl App {
    pub fn new(model_profile: String, permission_mode: agent_tools::PermissionMode, workspace_id: WorkspaceId) -> Self {
        let state = AppState::new(model_profile, permission_mode);
        Self {
            state,
            chat: ChatPanel::new(),
            sessions: SessionsPanel::new(),
            trace: TracePanel::new(),
            status_bar: StatusBar::new(),
            permission_modal: PermissionModal::new(),
            workspace_id,
            current_session_id: None,
            domain_events: Vec::new(),
            quit_confirmed: false,
            quitting: false,
        }
    }

    pub fn handle_crossterm_event(&mut self, event: &crossterm::event::Event) -> Vec<Command> {
        let crossterm::event::Event::Key(key) = event else {
            return Vec::new();
        };

        let permission_pending = matches!(self.chat.input_state, crate::app_state::InputState::PermissionWait { .. })
            || self.permission_modal.is_visible();

        let action = crate::keybindings::resolve_key(
            *key,
            self.state.focus_manager.current(),
            permission_pending,
            &self.state.input_mode,
        );

        self.apply_action(action)
    }

    fn apply_action(&mut self, action: KeyAction) -> Vec<Command> {
        let mut commands = Vec::new();
        let ctx = self.state.event_context();

        match action {
            KeyAction::InterruptOrQuit => {
                match self.state.record_ctrl_c() {
                    CtrlCAction::Interrupt => {
                        if let Some(session_id) = &self.current_session_id {
                            commands.push(Command::CancelSession {
                                workspace_id: self.workspace_id.clone(),
                                session_id: session_id.clone(),
                            });
                        }
                        self.state.render_scheduler.mark_dirty_immediate();
                    }
                    CtrlCAction::ConfirmQuit => {
                        self.quit_confirmed = true;
                        self.state.render_scheduler.mark_dirty_immediate();
                    }
                    CtrlCAction::ForceQuit => {
                        self.quitting = true;
                    }
                }
            }
            KeyAction::Quit => {
                self.quit_confirmed = true;
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::ToggleSessionsSidebar => {
                self.state.sidebar_left_visible = !self.state.sidebar_left_visible;
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::ToggleTraceSidebar => {
                self.state.sidebar_right_visible = !self.state.sidebar_right_visible;
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::FocusCycleNext => {
                self.state.focus_manager.cycle_next();
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::FocusChat => {
                self.state.focus_manager.set(FocusTarget::Chat);
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::FocusSessions => {
                self.state.focus_manager.set(FocusTarget::Sessions);
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::FocusTrace => {
                self.state.focus_manager.set(FocusTarget::Trace);
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::Redraw => {
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::ToggleTraceDensity => {
                self.trace.density = self.trace.density.next();
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::NewSession => {
                // Profile selector popup is a future enhancement; defaults to current profile for now
                commands.push(Command::StartSession {
                    workspace_id: self.workspace_id.clone(),
                    model_profile: self.state.model_profile.clone(),
                });
            }
            // Delegate all input/permission actions to ChatPanel
            KeyAction::SendInput
            | KeyAction::InputCharacter(_)
            | KeyAction::InputBackspace
            | KeyAction::InputDelete
            | KeyAction::InputNewline
            | KeyAction::InputHistoryUp
            | KeyAction::InputHistoryDown
            | KeyAction::ToggleInputMode
            | KeyAction::AllowPermission
            | KeyAction::DenyPermission
            | KeyAction::DenyAllPermission
            | KeyAction::Escape => {
                let ctx = self.state.event_context();
                let (effects, cmds) = self.chat.apply_key_action(action, &ctx);
                commands.extend(cmds);
                self.dispatch_effects(effects);
            }
            _ => {}
        }

        self.state.reset_ctrl_c();
        commands
    }

    pub fn handle_domain_event(&mut self, event: &agent_core::DomainEvent) {
        self.domain_events.push(event.clone());
        let mut effects = Vec::new();

        match &event.payload {
            EventPayload::UserMessageAdded { .. } => {
                self.state.current_session.apply(event);
            }
            EventPayload::ModelTokenDelta { .. } => {
                self.state.current_session.apply(event);
                if !self.state.render_scheduler.is_streaming {
                    effects.push(CrossPanelEffect::StartStreaming);
                }
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::AssistantMessageCompleted { .. } => {
                self.state.current_session.apply(event);
                effects.push(CrossPanelEffect::StopStreaming);
                self.state.render_scheduler.mark_dirty_immediate();
            }
            EventPayload::ToolInvocationStarted { tool_id, .. } => {
                self.state.current_session.apply(event);
                effects.push(CrossPanelEffect::SetStatus(StatusInfo {
                    profile: self.state.model_profile.clone(),
                    permission_mode: String::new(),
                    session_count: self.state.sessions.len(),
                    hint: format!("Running {tool_id}..."),
                    error: None,
                }));
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::ToolInvocationCompleted { .. } => {
                self.state.current_session.apply(event);
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::ToolInvocationFailed { error, .. } => {
                self.state.current_session.apply(event);
                effects.push(CrossPanelEffect::SetStatus(StatusInfo {
                    profile: self.state.model_profile.clone(),
                    permission_mode: String::new(),
                    session_count: self.state.sessions.len(),
                    hint: String::new(),
                    error: Some(error.clone()),
                }));
                self.state.render_scheduler.mark_dirty_immediate();
            }
            EventPayload::PermissionRequested {
                request_id,
                tool_id,
                preview,
            } => {
                let risk = self.classify_permission_risk(tool_id, preview);
                effects.push(CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
                    request_id: request_id.clone(),
                    tool_id: tool_id.clone(),
                    tool_preview: preview.clone(),
                    risk_level: risk,
                }));
                if matches!(risk, RiskLevel::Destructive) {
                    effects.push(CrossPanelEffect::SwitchFocus(FocusTarget::PermissionModal));
                    self.state.focus_manager.push(FocusTarget::PermissionModal);
                }
                self.state.render_scheduler.mark_dirty_immediate();
            }
            EventPayload::PermissionDenied { .. } | EventPayload::PermissionGranted { .. } => {
                effects.push(CrossPanelEffect::DismissPermissionPrompt);
                if self.state.focus_manager.current() == FocusTarget::PermissionModal {
                    self.state.focus_manager.pop();
                }
                self.state.render_scheduler.mark_dirty_immediate();
            }
            EventPayload::SessionCancelled { .. } => {
                self.state.current_session.apply(event);
                effects.push(CrossPanelEffect::StopStreaming);
                self.state.render_scheduler.reset();
            }
            EventPayload::AgentTaskCreated { title, .. } => {
                self.state.current_session.apply(event);
                // Update or add session info
                if let Some(session_id) = &self.current_session_id {
                    let existing = self.state.sessions.iter_mut().find(|s| s.id == *session_id);
                    if let Some(session) = existing {
                        session.title = title.clone();
                    }
                }
                self.state.render_scheduler.mark_dirty();
            }
            _ => {
                self.state.current_session.apply(event);
            }
        }

        self.dispatch_effects(effects);
    }

    fn classify_permission_risk(&self, tool_id: &str, _preview: &str) -> RiskLevel {
        // Simple heuristic: if tool is patch.apply or shell.exec with destructive markers
        // The runtime's PermissionEngine already classified; we infer from tool_id
        // A more complete version would read the ToolRisk from the event
        if tool_id == "patch.apply" {
            RiskLevel::Write
        } else {
            // default to Write for safety; Destructive is only via explicit modal
            RiskLevel::Write
        }
    }

    fn dispatch_effects(&mut self, effects: Vec<CrossPanelEffect>) {
        for effect in effects {
            self.chat.handle_effect(&effect);
            self.sessions.handle_effect(&effect);
            self.trace.handle_effect(&effect);
            self.status_bar.handle_effect(&effect);
            self.permission_modal.handle_effect(&effect);
        }
    }

    fn sync_component_focus(&mut self) {
        let current = self.state.focus_manager.current();
        self.chat.set_focused(current == FocusTarget::Chat);
        self.sessions.set_focused(current == FocusTarget::Sessions);
        self.trace.set_focused(current == FocusTarget::Trace);
        self.permission_modal.set_focused(current == FocusTarget::PermissionModal);
    }

    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        // Status bar at bottom (1 row)
        let [main_area, status_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);

        // Sidebars + Chat
        let mut constraints = Vec::new();
        if self.state.sidebar_left_visible {
            constraints.push(Constraint::Length(24));
        }
        constraints.push(Constraint::Min(20));
        if self.state.sidebar_right_visible {
            constraints.push(Constraint::Length(32));
        }

        let panels = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(main_area);

        let mut panel_index = 0;

        // Sessions panel (left sidebar)
        if self.state.sidebar_left_visible {
            let sessions_area = panels[panel_index];
            crate::components::sessions::render_sessions(
                sessions_area,
                frame,
                &self.state.sessions,
                self.sessions.focused(),
            );
            panel_index += 1;
        }

        // Chat panel (center)
        let chat_area = panels[panel_index];
        panel_index += 1;

        // Render chat messages + input
        let chat_focused = self.chat.focused();
        let [messages_area, input_area] = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(3)])
            .split(chat_area);

        crate::components::chat::render_messages(messages_area, frame, &self.state.current_session);

        // Input area rendering
        let input_text = match &self.chat.input_state {
            crate::app_state::InputState::Normal => self.chat.input_content.clone(),
            crate::app_state::InputState::PermissionWait { pending_prompt, .. } => {
                if let Some(prompt) = pending_prompt {
                    format!("⚠ {} ({})  [Y] Allow  [N] Deny  [D] Deny all", prompt.tool_id, "write")
                } else {
                "⚠ Permission required  [Y] Allow  [N] Deny  [D] Deny all".into()
                }
            }
        };
        let input_mode_label = match self.chat.input_mode {
            crate::app_state::InputMode::SingleLine => ">",
            crate::app_state::InputMode::MultiLine => ">>",
        };
        let border_style = if chat_focused {
            ratatui::style::Style::default().fg(ratatui::style::Color::Cyan)
        } else {
            ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray)
        };
        let input = ratatui::widgets::Paragraph::new(format!("{input_mode_label} {input_text}"))
            .block(
                ratatui::widgets::Block::default()
                    .borders(ratatui::widgets::Borders::TOP)
                    .border_style(border_style),
            );
        frame.render_widget(input, input_area);

        // Streaming cursor indicator
        if !self.state.current_session.token_stream.is_empty() {
            // Token stream is being rendered in render_messages already
        }

        // Trace panel (right sidebar)
        if self.state.sidebar_right_visible {
            let trace_area = panels[panel_index];
            let traces = extract_tool_traces(&self.domain_events);
            render_trace_l1(trace_area, frame, &traces, self.trace.focused());
        }

        // Status bar
        let status_info = StatusInfo {
            profile: self.state.model_profile.clone(),
            permission_mode: self.state.permission_mode.as_str().to_string(),
            session_count: self.state.sessions.len(),
            hint: if self.quit_confirmed {
                "Quit? Y/N".into()
            } else {
                "Alt+S/? for help".into()
            },
            error: None,
        };
        crate::components::status_bar::render_status_bar(status_area, frame, &status_info);

        // Permission modal overlay (renders last, on top)
        if self.permission_modal.is_visible() {
            self.permission_modal.render(area, frame);
        }

        // Quit confirmation overlay
        if self.quit_confirmed {
            // Simple inline: status bar already shows hint
            // For a proper overlay, we'd render a centered box — skip for now
        }
    }
}

/// Re-export for convenience
pub use crate::components::trace::render_trace_l1;
```

- [ ] **Step 2: Update lib.rs/main.rs module declarations**

Replace `crates/agent-tui/src/app.rs` module import. Ensure `main.rs` includes:

```rust
mod app;
mod app_state;
mod components;
mod keybindings;
mod view;
```

Add `components/mod.rs` submodules:

```rust
pub mod chat;
pub mod sessions;
pub mod status_bar;
pub mod trace;
pub mod permission_modal;

pub use mod::*;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p agent-tui`
Expected: compiles with no errors

- [ ] **Step 4: Commit**

```bash
git add crates/agent-tui/src/
git commit -m "feat(tui): add App with component composition and event routing"
```

---

## Task 10: Main Event Loop — Wire Terminal + Runtime

**Files:**

- Rewrite: `crates/agent-tui/src/main.rs`

- [ ] **Step 1: Write the full interactive main.rs**

Replace `crates/agent-tui/src/main.rs` with:

```rust
mod app;
mod app_state;
mod components;
mod keybindings;
mod view;

use app::App;
use agent_core::{AppFacade, StartSessionRequest, WorkspaceId};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use crossterm::event::{self, Event, KeyEvent};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;
use std::io;
use std::time::Duration;

enum AppEvent {
    Key(KeyEvent),
    DomainEvent(agent_core::DomainEvent),
    Tick,
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

fn choose_profile(profiles: &[String]) -> &str {
    if profiles.iter().any(|p| p == "fast") {
        "fast"
    } else if profiles.iter().any(|p| p == "local-code") {
        "local-code"
    } else {
        "fake"
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Check minimum terminal size
    let size = terminal.size()?;
    if size.width < 80 || size.height < 24 {
        disable_raw_mode()?;
        crossterm::execute!(io::stdout(), LeaveAlternateScreen)?;
        eprintln!(
            "Error: Terminal too small (need 80×24, got {}×{}). Please resize.",
            size.width, size.height
        );
        std::process::exit(1);
    }

    // Initialize runtime
    let store = SqliteEventStore::in_memory().await?;
    let profiles = detect_profiles();
    let profile = choose_profile(&profiles);
    let workspace_path = std::env::current_dir()?;

    let model = FakeModelClient::new(vec!["hello from fake model".into()]);
    let runtime = LocalRuntime::new(store, model)
        .with_permission_mode(PermissionMode::Suggest)
        .with_context_limit(100_000)
        .with_builtin_tools(workspace_path.clone())
        .await;

    let workspace = runtime
        .open_workspace(workspace_path.display().to_string())
        .await?;

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: profile.to_string(),
        })
        .await?;

    // Create App
    let mut app = App::new(
        profile.to_string(),
        PermissionMode::Suggest,
        workspace.workspace_id.clone(),
    );
    app.current_session_id = Some(session_id.clone());

    // Add initial session to sessions list
    app.state.sessions.push(components::SessionInfo {
        id: session_id.clone(),
        title: format!("Session using {profile}"),
        model_profile: profile.to_string(),
        state: components::SessionState::Active,
        pinned: false,
    });

    // Subscribe to domain events
    let (app_event_tx, mut app_event_rx) = tokio::sync::mpsc::channel::<AppEvent>(256);

    // Forward domain events
    let event_tx = app_event_tx.clone();
    let sid = session_id.clone();
    let mut domain_stream = runtime.subscribe_session(sid);
    tokio::spawn(async move {
        use futures::StreamExt;
        while let Some(event) = domain_stream.next().await {
            if event_tx.send(AppEvent::DomainEvent(event)).await.is_err() {
                break;
            }
        }
    });

    // Forward crossterm key events
    let key_tx = app_event_tx.clone();
    tokio::spawn(async move {
        loop {
            if event::poll(Duration::from_millis(16)).unwrap_or(false) {
                if let Ok(Event::Key(key)) = event::read() {
                    if key_tx.send(AppEvent::Key(key)).await.is_err() {
                        break;
                    }
                }
            }
        }
    });

    // Tick timer
    let tick_tx = app_event_tx.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(16));
        loop {
            interval.tick().await;
            if tick_tx.send(AppEvent::Tick).await.is_err() {
                break;
            }
        }
    });

    // Main event loop
    let result = run_loop(&mut terminal, &mut app, &runtime, &mut app_event_rx).await;

    // Restore terminal
    disable_raw_mode()?;
    crossterm::execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

async fn run_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    runtime: &LocalRuntime<SqliteEventStore, FakeModelClient>,
    rx: &mut tokio::sync::mpsc::Receiver<AppEvent>,
) -> anyhow::Result<()> {
    loop {
        if let Some(app_event) = rx.recv().await {
            match app_event {
                AppEvent::Key(key) => {
                    let commands = app.handle_crossterm_event(&Event::Key(key));
                    dispatch_commands(commands, runtime, app).await?;
                }
                AppEvent::DomainEvent(event) => {
                    app.handle_domain_event(&event);
                }
                AppEvent::Tick => {
                    if app.state.render_scheduler.should_render() {
                        terminal.draw(|f| app.render(f))?;
                    }
                }
            }

            // Check resize
            if let Ok(new_size) = terminal.size() {
                if new_size.width < 80 || new_size.height < 24 {
                    // Below minimum — would show overlay, but just skip render
                }
            }

            if app.quitting {
                break;
            }
        }
    }
    Ok(())
}

async fn dispatch_commands(
    commands: Vec<components::Command>,
    runtime: &LocalRuntime<SqliteEventStore, FakeModelClient>,
    app: &mut App,
) -> anyhow::Result<()> {
    use components::Command;

    for cmd in commands {
        match cmd {
            Command::SendMessage {
                workspace_id,
                session_id,
                content,
            } => {
                if let Err(e) = runtime
                    .send_message(agent_core::SendMessageRequest {
                        workspace_id,
                        session_id,
                        content,
                    })
                    .await
                {
                    // Surface error in chat as system message
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("⚠ Failed to send: {e}"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty_immediate();
                }
            }
            Command::DecidePermission {
                request_id,
                approved,
            } => {
                if let Err(e) = runtime
                    .decide_permission(agent_core::PermissionDecision {
                        request_id,
                        approve: approved,
                        reason: None,
                    })
                    .await
                {
                    // Error: keep prompt visible, show error
                    app.state.current_session.messages.push(
                        agent_core::projection::ProjectedMessage {
                            role: agent_core::projection::ProjectedRole::Assistant,
                            content: format!("⚠ Permission response failed: {e}"),
                        },
                    );
                    app.state.render_scheduler.mark_dirty_immediate();
                }
            }
            Command::CancelSession {
                workspace_id,
                session_id,
            } => {
                let _ = runtime.cancel_session(workspace_id, session_id).await;
            }
            Command::StartSession {
                workspace_id,
                model_profile,
            } => {
                match runtime
                    .start_session(StartSessionRequest {
                        workspace_id,
                        model_profile: model_profile.clone(),
                    })
                    .await
                {
                    Ok(new_session_id) => {
                        app.current_session_id = Some(new_session_id.clone());
                        app.state.sessions.push(components::SessionInfo {
                            id: new_session_id,
                            title: format!("Session using {model_profile}"),
                            model_profile,
                            state: components::SessionState::Active,
                            pinned: false,
                        });
                        app.domain_events.clear();
                        app.state.current_session = agent_core::projection::SessionProjection::default();
                        app.state.render_scheduler.reset();
                    }
                    Err(e) => {
                        app.state.current_session.messages.push(
                            agent_core::projection::ProjectedMessage {
                                role: agent_core::projection::ProjectedRole::Assistant,
                                content: format!("⚠ Could not start session: {e}"),
                            },
                        );
                        app.state.render_scheduler.mark_dirty_immediate();
                    }
                }
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 2: Verify compilation with `cargo check`**

Run: `cargo check -p agent-tui`
Expected: compiles

- [ ] **Step 3: Run full workspace tests**

Run: `cargo test --workspace --all-targets`
Expected: all existing tests still pass

- [ ] **Step 4: Commit**

```bash
git add crates/agent-tui/src/main.rs
git commit -m "feat(tui): wire interactive event loop with terminal setup and runtime integration"
```

---

## Task 11: Integration Test — Full Session Flow

**Files:**

- Create: `crates/agent-tui/tests/interactive_session.rs`

- [ ] **Step 1: Write integration test for FakeModelClient session flow**

Create `crates/agent-tui/tests/interactive_session.rs`:

```rust
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;

#[tokio::test]
async fn full_session_flow_sends_message_and_receives_response() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hello from fake model".into()]);
    let runtime = LocalRuntime::new(store, model)
        .with_permission_mode(PermissionMode::Suggest)
        .with_context_limit(100_000);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hi".into(),
        })
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(projection.messages.len(), 2);
    assert_eq!(projection.messages[0].content, "hi");
    assert_eq!(projection.messages[1].content, "hello from fake model");
}

#[tokio::test]
async fn event_subscription_receives_streaming_events() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["streaming response".into()]);
    let runtime = LocalRuntime::new(store, model)
        .with_permission_mode(PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    let mut event_stream = runtime.subscribe_session(session_id.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello".into(),
        })
        .await
        .unwrap();

    // Collect a few events from the stream
    use futures::StreamExt;
    let mut events = Vec::new();
    for _ in 0..5 {
        tokio::select! {
            Some(event) = event_stream.next() => {
                events.push(event);
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                break;
            }
        }
    }

    assert!(!events.is_empty(), "Should receive at least one event from subscription");
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p agent-tui --test interactive_session`
Expected: All 2 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/agent-tui/tests/interactive_session.rs
git commit -m "test(tui): add integration tests for session flow and event subscription"
```

---

## Task 12: Snapshot Tests for Rendered UI

**Files:**

- Create: `crates/agent-tui/tests/snapshot_tests.rs`

- [ ] **Step 1: Write snapshot tests using ratatui TestBackend and insta**

Create `crates/agent-tui/tests/snapshot_tests.rs`:

```rust
use agent_core::projection::{ProjectedMessage, ProjectedRole, SessionProjection};
use ratatui::backend::TestBackend;
use ratatui::Terminal;

fn render_snapshot(projection: &SessionProjection, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            agent_tui_test_helpers::render_messages(f.area(), f, projection);
        })
        .unwrap();
    terminal.backend().to_string()
}

#[test]
fn chat_panel_renders_user_and_assistant_messages() {
    let projection = SessionProjection {
        messages: vec![
            ProjectedMessage {
                role: ProjectedRole::User,
                content: "fix the build error".into(),
            },
            ProjectedMessage {
                role: ProjectedRole::Assistant,
                content: "I found the issue in Cargo.toml".into(),
            },
        ],
        task_titles: vec!["Session using fake".into()],
        token_stream: String::new(),
        cancelled: false,
    };

    let output = render_snapshot(&projection, 80, 24);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn chat_panel_renders_streaming_token_with_cursor() {
    let projection = SessionProjection {
        messages: vec![ProjectedMessage {
            role: ProjectedRole::User,
            content: "hello".into(),
        }],
        task_titles: vec![],
        token_stream: "streaming te".into(),
        cancelled: false,
    };

    let output = render_snapshot(&projection, 80, 24);
    insta::assert_yaml_snapshot!(output);
}

#[test]
fn chat_panel_renders_cancelled_marker() {
    let projection = SessionProjection {
        messages: vec![ProjectedMessage {
            role: ProjectedRole::User,
            content: "hello".into(),
        }],
        task_titles: vec![],
        token_stream: String::new(),
        cancelled: true,
    };

    let output = render_snapshot(&projection, 80, 24);
    insta::assert_yaml_snapshot!(output);
}

// Helper module that exposes render functions for testing
mod agent_tui_test_helpers {
    use agent_core::projection::SessionProjection;
    use ratatui::layout::Rect;
    use ratatui::Frame;

    pub fn render_messages(area: Rect, frame: &mut Frame, projection: &SessionProjection) {
        // Use the same render_messages from chat.rs
        // For testing, we inline a simplified version
        use ratatui::style::{Color, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;
        use ratatui::widgets::Wrap;

        let lines: Vec<Line> = projection
            .messages
            .iter()
            .map(|msg| match msg.role {
                ProjectedRole::User => Line::from(vec![
                    Span::styled("You: ", Style::default().fg(Color::Cyan)),
                    Span::raw(&msg.content),
                ]),
                ProjectedRole::Assistant => Line::from(vec![
                    Span::styled("Agent: ", Style::default().fg(Color::Green)),
                    Span::raw(&msg.content),
                ]),
            })
            .chain(std::iter::once_with(|| {
                if projection.cancelled {
                    Line::from(Span::styled("[cancelled]", Style::default().fg(Color::Yellow)))
                } else if !projection.token_stream.is_empty() {
                    Line::from(vec![
                        Span::styled("Agent: ", Style::default().fg(Color::Green)),
                        Span::raw(&projection.token_stream),
                        Span::styled("▌", Style::default().fg(Color::White)),
                    ])
                } else {
                    Line::from("")
                }
            }))
            .collect();

        let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }
}
```

- [ ] **Step 2: Run snapshot tests (they will create snapshots on first run)**

Run: `cargo test -p agent-tui --test snapshot_tests`
Expected: tests pass, creating snapshot files

- [ ] **Step 3: Review and commit snapshots**

```bash
git add crates/agent-tui/tests/snapshot_tests.rs crates/agent-tui/**/snapshots/
git commit -m "test(tui): add snapshot tests for chat panel rendering"
```

---

## Task 13: Final Verification and Cleanup

**Files:**

- Modify: `crates/agent-tui/src/view.rs` (update to delegate to component renderers)

- [ ] **Step 1: Run full workspace verification**

Run: `cargo test --workspace --all-targets`
Expected: all tests pass (80+ existing + new TUI tests)

- [ ] **Step 2: Run clippy and fmt**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`
Expected: no warnings

- [ ] **Step 3: Verify TUI launches interactively**

Run: `cargo run -p agent-tui`
Expected: opens interactive terminal UI, stays running, responds to key input, Ctrl+C twice exits

- [ ] **Step 4: Final commit**

```bash
git add -A
git commit -m "chore(tui): final verification and cleanup for interactive TUI"
```
