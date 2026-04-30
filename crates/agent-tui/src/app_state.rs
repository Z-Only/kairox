use std::time::{Duration, Instant};

use agent_core::projection::SessionProjection;
use agent_tools::PermissionMode;

use crate::components::{FocusTarget, SessionInfo};

// ---------------------------------------------------------------------------
// FocusManager
// ---------------------------------------------------------------------------

/// Stack-based focus management for the TUI.
///
/// The top of the stack is the currently focused target. Modal overlays
/// (e.g., `PermissionModal`) are pushed on top and restored on pop.
#[derive(Debug)]
pub struct FocusManager {
    stack: Vec<FocusTarget>,
}

impl FocusManager {
    pub fn new(default: FocusTarget) -> Self {
        Self {
            stack: vec![default],
        }
    }

    /// Return the currently focused target (top of the stack).
    pub fn current(&self) -> FocusTarget {
        *self
            .stack
            .last()
            .expect("FocusManager stack must never be empty")
    }

    /// Push a modal focus target on top of the stack.
    pub fn push(&mut self, target: FocusTarget) {
        self.stack.push(target);
    }

    /// Pop the top focus target. Returns `None` if only one element remains
    /// (we never empty the stack). Returns the popped target otherwise.
    pub fn pop(&mut self) -> Option<FocusTarget> {
        if self.stack.len() <= 1 {
            None
        } else {
            self.stack.pop()
        }
    }

    /// Tab cycling: Chat → Sessions → Trace → Chat …
    /// If a modal (PermissionModal) is on top, cycling is a no-op.
    pub fn cycle_next(&mut self) {
        if self.stack.is_empty() {
            return;
        }

        if self.current() == FocusTarget::PermissionModal {
            return; // don't cycle while modal is focused
        }

        let next = match self.current() {
            FocusTarget::Chat => FocusTarget::Sessions,
            FocusTarget::Sessions => FocusTarget::Trace,
            FocusTarget::Trace => FocusTarget::Chat,
            FocusTarget::PermissionModal => unreachable!(),
        };

        let last = self
            .stack
            .last_mut()
            .expect("FocusManager stack must never be empty");
        *last = next;
    }

    /// Directly set focus (for Alt+1/2/3 shortcuts).
    /// Replaces the top of the stack.
    pub fn set(&mut self, target: FocusTarget) {
        let last = self
            .stack
            .last_mut()
            .expect("FocusManager stack must never be empty");
        *last = target;
    }
}

// ---------------------------------------------------------------------------
// RenderScheduler
// ---------------------------------------------------------------------------

/// Adaptive frame-rate throttling for the TUI render loop.
///
/// In non-streaming mode we render at ~60 fps (16 ms). During streaming,
/// the interval is adapted based on the number of tokens accumulated since
/// the last render to avoid burning CPU on rapid small updates.
#[derive(Debug)]
pub struct RenderScheduler {
    /// Base interval (16 ms ≈ 60 fps).
    base_interval: Duration,
    /// Current adaptive interval.
    interval: Duration,
    /// Whether state has changed since the last render.
    dirty: bool,
    /// Whether we are in streaming mode.
    streaming: bool,
    /// Number of tokens that arrived since the last render.
    tokens_since_render: usize,
    /// Time of the last render.
    last_render: Instant,
}

impl RenderScheduler {
    const BASE_INTERVAL_MS: u64 = 16;
    const STREAMING_FAST_TOKENS: usize = 5;
    const STREAMING_FAST_INTERVAL_MS: u64 = 60;
    const STREAMING_SLOW_TOKENS: usize = 20;
    const STREAMING_SLOW_INTERVAL_MS: u64 = 120;

    pub fn new() -> Self {
        Self {
            base_interval: Duration::from_millis(Self::BASE_INTERVAL_MS),
            interval: Duration::from_millis(Self::BASE_INTERVAL_MS),
            dirty: true,
            streaming: false,
            tokens_since_render: 0,
            last_render: Instant::now(),
        }
    }

    /// Mark state as changed — a render is needed.
    pub fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    /// Mark dirty and immediately boost to the fastest frame rate
    /// (used for key presses, resize events).
    pub fn mark_dirty_immediate(&mut self) {
        self.dirty = true;
        self.interval = Duration::from_millis(Self::BASE_INTERVAL_MS);
    }

    /// Check whether we are currently in streaming mode.
    pub fn is_streaming(&self) -> bool {
        self.streaming
    }

    /// Enter or exit streaming mode.
    pub fn set_streaming(&mut self, streaming: bool) {
        self.streaming = streaming;
        if !streaming {
            self.tokens_since_render = 0;
            self.interval = self.base_interval;
        }
    }

    /// Record that tokens have arrived. Call this from the token-delta handler.
    pub fn add_tokens(&mut self, count: usize) {
        self.tokens_since_render += count;
    }

    /// Check whether we should render now.
    ///
    /// Returns `true` when the state is dirty **and** enough time has elapsed
    /// according to the adaptive interval. After returning `true`, the caller
    /// should call [`RenderScheduler::did_render`] to reset the timer and
    /// counters.
    pub fn should_render(&mut self) -> bool {
        if !self.dirty {
            return false;
        }

        self.update_interval();

        let elapsed = self.last_render.elapsed();
        elapsed >= self.interval
    }

    /// Call after a render has been performed.
    pub fn did_render(&mut self) {
        self.dirty = false;
        self.tokens_since_render = 0;
        self.last_render = Instant::now();
    }

    /// Reset all counters and state.
    pub fn reset(&mut self) {
        self.interval = self.base_interval;
        self.dirty = true;
        self.streaming = false;
        self.tokens_since_render = 0;
        self.last_render = Instant::now();
    }

    fn update_interval(&mut self) {
        if !self.streaming {
            self.interval = self.base_interval;
            return;
        }

        // Adaptive: ≥20 tokens → 120 ms, ≥5 tokens → 60 ms, else 16 ms.
        self.interval = if self.tokens_since_render >= Self::STREAMING_SLOW_TOKENS {
            Duration::from_millis(Self::STREAMING_SLOW_INTERVAL_MS)
        } else if self.tokens_since_render >= Self::STREAMING_FAST_TOKENS {
            Duration::from_millis(Self::STREAMING_FAST_INTERVAL_MS)
        } else {
            self.base_interval
        };
    }
}

impl Default for RenderScheduler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// InputMode / InputState / CtrlCAction
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    SingleLine,
    MultiLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputState {
    Normal,
    PermissionWait {
        request_id: String,
        pending_prompt: String,
    },
}

/// Result of recording a Ctrl-C key press.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CtrlCAction {
    /// First press — signal the agent to stop.
    Interrupt,
    /// Second press within 5 s — confirm the user really wants to quit.
    ConfirmQuit,
    /// Third press within 2 s — force quit immediately.
    ForceQuit,
}

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// Central shared state for the interactive TUI.
pub struct AppState {
    pub focus_manager: FocusManager,
    pub render_scheduler: RenderScheduler,

    // Sidebar visibility
    pub sidebar_left_visible: bool,
    pub sidebar_right_visible: bool,

    // Session data
    pub current_session: SessionProjection,
    pub sessions: Vec<SessionInfo>,

    // Model / permissions
    pub model_profile: String,
    pub permission_mode: PermissionMode,

    // Input
    pub input_mode: InputMode,
    pub input_state: InputState,
    pub input_content: String,
    pub input_cursor: usize,
    pub input_history: Vec<String>,
    pub input_history_index: usize,

    // Ctrl-C progressive exit
    ctrl_c_count: u8,
    last_ctrl_c: Option<Instant>,
}

impl AppState {
    const CTRL_C_CONFIRM_WINDOW: Duration = Duration::from_secs(5);
    const CTRL_C_FORCE_WINDOW: Duration = Duration::from_secs(2);

    pub fn new(model_profile: impl Into<String>, permission_mode: PermissionMode) -> Self {
        Self {
            focus_manager: FocusManager::new(FocusTarget::Chat),
            render_scheduler: RenderScheduler::new(),
            sidebar_left_visible: true,
            sidebar_right_visible: false,
            current_session: SessionProjection::default(),
            sessions: Vec::new(),
            model_profile: model_profile.into(),
            permission_mode,
            input_mode: InputMode::SingleLine,
            input_state: InputState::Normal,
            input_content: String::new(),
            input_cursor: 0,
            input_history: Vec::new(),
            input_history_index: 0,
            ctrl_c_count: 0,
            last_ctrl_c: None,
        }
    }

    /// Build a borrow of `EventContext` from the current state.
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

    /// Record a Ctrl-C press and return the progressive action.
    ///
    /// | Press | Timing                    | Action      |
    /// |-------|---------------------------|-------------|
    /// | 1st   | —                         | Interrupt   |
    /// | 2nd   | ≤ 5 s after 1st           | ConfirmQuit |
    /// | 3rd   | ≤ 2 s after 2nd           | ForceQuit   |
    ///
    /// After 5 s of inactivity the counter resets to 0.
    pub fn record_ctrl_c(&mut self) -> CtrlCAction {
        let now = Instant::now();

        // If more than 5 s since the last Ctrl-C, reset the counter.
        if let Some(last) = self.last_ctrl_c {
            if now.duration_since(last) > Self::CTRL_C_CONFIRM_WINDOW {
                self.ctrl_c_count = 0;
            }
        }

        self.ctrl_c_count += 1;

        let prev_time = self.last_ctrl_c;
        self.last_ctrl_c = Some(now);

        match self.ctrl_c_count {
            1 => CtrlCAction::Interrupt,
            2 => CtrlCAction::ConfirmQuit,
            _ => {
                // 3rd (or more) press — require ≤ 2 s gap from previous press
                if let Some(last) = prev_time {
                    if now.duration_since(last) <= Self::CTRL_C_FORCE_WINDOW {
                        CtrlCAction::ForceQuit
                    } else {
                        // Too slow — treat as a fresh 1st press
                        self.ctrl_c_count = 1;
                        CtrlCAction::Interrupt
                    }
                } else {
                    CtrlCAction::ForceQuit
                }
            }
        }
    }

    /// Reset the Ctrl-C counter and timer.
    pub fn reset_ctrl_c(&mut self) {
        self.ctrl_c_count = 0;
        self.last_ctrl_c = None;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // -- FocusManager -------------------------------------------------------

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
        assert_eq!(fm.current(), FocusTarget::Chat);

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

        fm.set(FocusTarget::Trace);
        assert_eq!(fm.current(), FocusTarget::Trace);

        fm.push(FocusTarget::PermissionModal);
        assert_eq!(fm.current(), FocusTarget::PermissionModal);

        fm.set(FocusTarget::Sessions);
        assert_eq!(fm.current(), FocusTarget::Sessions);

        assert_eq!(fm.pop(), Some(FocusTarget::Sessions));
        assert_eq!(fm.current(), FocusTarget::Trace);
    }

    // -- RenderScheduler ----------------------------------------------------

    #[test]
    fn render_scheduler_adapts_interval_during_streaming() {
        let mut rs = RenderScheduler::new();
        rs.set_streaming(true);

        rs.add_tokens(4);
        rs.mark_dirty();
        rs.last_render = Instant::now() - Duration::from_millis(200);
        let _ = rs.should_render();
        assert_eq!(rs.interval, Duration::from_millis(16));

        rs.add_tokens(2);
        rs.mark_dirty();
        rs.last_render = Instant::now() - Duration::from_millis(200);
        let _ = rs.should_render();
        assert_eq!(rs.interval, Duration::from_millis(60));

        rs.did_render();
        rs.set_streaming(true);
        rs.add_tokens(20);
        rs.mark_dirty();
        rs.last_render = Instant::now() - Duration::from_millis(200);
        let _ = rs.should_render();
        assert_eq!(rs.interval, Duration::from_millis(120));
    }

    #[test]
    fn render_scheduler_non_streaming_is_fast() {
        let mut rs = RenderScheduler::new();
        assert!(!rs.streaming);
        rs.add_tokens(100);
        rs.mark_dirty();
        rs.last_render = Instant::now() - Duration::from_millis(200);
        let _ = rs.should_render();
        assert_eq!(rs.interval, Duration::from_millis(16));
    }

    // -- Ctrl-C progressive exit --------------------------------------------

    #[test]
    fn ctrl_c_progressive_exit_interrupt_then_confirm_then_force() {
        let mut state = AppState::new("fake", PermissionMode::Suggest);

        assert_eq!(state.record_ctrl_c(), CtrlCAction::Interrupt);
        assert_eq!(state.record_ctrl_c(), CtrlCAction::ConfirmQuit);
        assert_eq!(state.record_ctrl_c(), CtrlCAction::ForceQuit);
    }

    #[test]
    fn ctrl_c_resets_after_timeout() {
        let mut state = AppState::new("fake", PermissionMode::Suggest);

        assert_eq!(state.record_ctrl_c(), CtrlCAction::Interrupt);

        state.last_ctrl_c = Some(Instant::now() - Duration::from_secs(6));

        assert_eq!(state.record_ctrl_c(), CtrlCAction::Interrupt);
        assert_eq!(state.ctrl_c_count, 1);
    }
}
