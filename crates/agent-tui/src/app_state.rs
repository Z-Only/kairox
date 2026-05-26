use std::time::{Duration, Instant};

use agent_core::projection::SessionProjection;
use agent_core::{ConfigScope, ProjectId};
use agent_tools::{ApprovalPolicy, PermissionMode, SandboxPolicy};

use crate::components::{FocusTarget, ProjectInfo, SessionInfo};

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

        if matches!(
            self.current(),
            FocusTarget::PermissionModal
                | FocusTarget::McpOverlay
                | FocusTarget::CommandPalette
                | FocusTarget::SkillsOverlay
                | FocusTarget::ModelOverlay
                | FocusTarget::AgentOverlay
                | FocusTarget::PluginOverlay
                | FocusTarget::HooksOverlay
                | FocusTarget::InstructionsOverlay
        ) {
            return; // don't cycle while a modal is focused
        }

        let next = match self.current() {
            FocusTarget::Chat => FocusTarget::Sessions,
            FocusTarget::Sessions => FocusTarget::Trace,
            FocusTarget::Trace => FocusTarget::Chat,
            FocusTarget::PermissionModal
            | FocusTarget::McpOverlay
            | FocusTarget::CommandPalette
            | FocusTarget::SkillsOverlay
            | FocusTarget::ModelOverlay
            | FocusTarget::AgentOverlay
            | FocusTarget::PluginOverlay
            | FocusTarget::HooksOverlay
            | FocusTarget::InstructionsOverlay => unreachable!(),
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
// StatusLog
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusLogEntry {
    pub message: String,
}

// ---------------------------------------------------------------------------
// SettingsConfigSource
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SettingsConfigSource {
    #[default]
    User,
    Project,
}

impl SettingsConfigSource {
    pub fn as_filter(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Project => "project",
        }
    }

    pub fn as_scope(self) -> ConfigScope {
        match self {
            Self::User => ConfigScope::User,
            Self::Project => ConfigScope::Project,
        }
    }
}

// ---------------------------------------------------------------------------
// AppState
// ---------------------------------------------------------------------------

/// Central shared state for the interactive TUI.
#[allow(dead_code)]
pub struct AppState {
    pub focus_manager: FocusManager,
    pub render_scheduler: RenderScheduler,

    // Sidebar visibility
    pub sidebar_left_visible: bool,
    pub sidebar_right_visible: bool,

    // Session data
    pub current_session: SessionProjection,
    pub sessions: Vec<SessionInfo>,
    pub projects: Vec<ProjectInfo>,

    // Settings source selection
    settings_config_source: SettingsConfigSource,
    settings_project_id: Option<ProjectId>,

    // Model / permissions
    pub model_profile: String,
    /// Latest reasoning effort for the active session, mirrored from
    /// `EventPayload::ModelProfileSwitched.reasoning_effort`. `None` until the
    /// first switch event lands, or for non-reasoning profiles.
    pub reasoning_effort: Option<String>,
    /// Legacy single-axis permission mode (PR-2e will remove this in favor of
    /// the orthogonal approval × sandbox model below).
    pub permission_mode: PermissionMode,
    /// Approval axis of the double-axis policy. Mirrored to the active session
    /// via [`agent_runtime::AppFacade::set_session_approval_policy`].
    pub approval_policy: ApprovalPolicy,
    /// Sandbox axis of the double-axis policy. Mirrored to the active session
    /// via [`agent_runtime::AppFacade::set_session_sandbox_policy`].
    pub sandbox_policy: SandboxPolicy,

    // Input
    pub input_mode: InputMode,
    pub input_state: InputState,
    pub input_content: String,
    pub input_cursor: usize,
    pub input_history: Vec<String>,
    pub input_history_index: usize,

    // Local operation feedback
    pub status_log: Vec<StatusLogEntry>,

    // Ctrl-C progressive exit
    ctrl_c_count: u8,
    last_ctrl_c: Option<Instant>,
}

impl AppState {
    const STATUS_LOG_LIMIT: usize = 100;
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
            projects: Vec::new(),
            settings_config_source: SettingsConfigSource::User,
            settings_project_id: None,
            model_profile: model_profile.into(),
            reasoning_effort: None,
            permission_mode,
            approval_policy: ApprovalPolicy::default(),
            sandbox_policy: SandboxPolicy::default(),
            input_mode: InputMode::SingleLine,
            input_state: InputState::Normal,
            input_content: String::new(),
            input_cursor: 0,
            input_history: Vec::new(),
            input_history_index: 0,
            status_log: Vec::new(),
            ctrl_c_count: 0,
            last_ctrl_c: None,
        }
    }

    pub fn settings_config_source(&self) -> SettingsConfigSource {
        self.settings_config_source
    }

    pub fn set_settings_config_source(&mut self, source: SettingsConfigSource) {
        self.settings_config_source = source;
    }

    pub fn settings_scope(&self) -> ConfigScope {
        self.settings_config_source.as_scope()
    }

    pub fn settings_source_filter(&self) -> Option<String> {
        Some(self.settings_config_source.as_filter().to_string())
    }

    pub fn select_settings_project(&mut self, project_id: ProjectId) {
        self.settings_project_id = Some(project_id);
    }

    pub fn selected_settings_project_id(&self) -> Option<&ProjectId> {
        self.settings_project_id.as_ref()
    }

    pub fn selected_settings_project(&self) -> Option<&ProjectInfo> {
        if self.settings_config_source != SettingsConfigSource::Project {
            return None;
        }
        self.settings_project_id
            .as_ref()
            .and_then(|project_id| {
                self.projects
                    .iter()
                    .find(|project| &project.id == project_id)
            })
            .or_else(|| self.projects.first())
    }

    pub fn selected_settings_project_root(&self) -> Option<std::path::PathBuf> {
        self.selected_settings_project()
            .map(|project| std::path::PathBuf::from(&project.root_path))
    }

    pub fn selected_settings_project_config_path(&self) -> Option<std::path::PathBuf> {
        self.selected_settings_project_root()
            .map(|root| root.join(".kairox").join("config.toml"))
    }

    /// Build a borrow of `EventContext` from the current state.
    #[allow(dead_code)]
    pub fn event_context<'a>(
        &'a self,
        workspace_id: &'a agent_core::WorkspaceId,
        current_session_id: &'a Option<agent_core::SessionId>,
    ) -> crate::components::EventContext<'a> {
        crate::components::EventContext {
            focus: self.focus_manager.current(),
            current_session: &self.current_session,
            projects: &self.projects,
            sessions: &self.sessions,
            model_profile: &self.model_profile,
            permission_mode: self.permission_mode,
            sidebar_left_visible: self.sidebar_left_visible,
            sidebar_right_visible: self.sidebar_right_visible,
            workspace_id,
            current_session_id,
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

    /// Advance the active permission mode to the next value in the cycle.
    /// Returns the new mode so the caller can dispatch it to the runtime.
    pub fn cycle_permission_mode(&mut self) -> PermissionMode {
        use crate::components::status_bar::PermissionModeExt;
        self.permission_mode = self.permission_mode.next();
        self.permission_mode
    }

    /// Advance the active approval policy to the next value in the cycle.
    /// Order: OnRequest → Always → Never → OnRequest.
    pub fn cycle_approval_policy(&mut self) -> ApprovalPolicy {
        self.approval_policy = match self.approval_policy {
            ApprovalPolicy::OnRequest => ApprovalPolicy::Always,
            ApprovalPolicy::Always => ApprovalPolicy::Never,
            ApprovalPolicy::Never => ApprovalPolicy::OnRequest,
        };
        self.approval_policy
    }

    /// Advance the active sandbox policy to the next value in the cycle.
    /// Order: WorkspaceWrite → DangerFullAccess → ReadOnly → WorkspaceWrite.
    /// Cycling uses the default `WorkspaceWrite` (no network, no extra
    /// writable roots); fine-grained tuning lives in config files.
    pub fn cycle_sandbox_policy(&mut self) -> SandboxPolicy {
        self.sandbox_policy = match &self.sandbox_policy {
            SandboxPolicy::WorkspaceWrite { .. } => SandboxPolicy::DangerFullAccess,
            SandboxPolicy::DangerFullAccess => SandboxPolicy::ReadOnly,
            SandboxPolicy::ReadOnly => SandboxPolicy::default(),
        };
        self.sandbox_policy.clone()
    }

    pub fn push_status_message(&mut self, message: impl Into<String>) {
        let message = message.into();
        if message.trim().is_empty() {
            return;
        }
        self.status_log.push(StatusLogEntry { message });
        if self.status_log.len() > Self::STATUS_LOG_LIMIT {
            let overflow = self.status_log.len() - Self::STATUS_LOG_LIMIT;
            self.status_log.drain(0..overflow);
        }
    }

    pub fn latest_status_message(&self) -> Option<&StatusLogEntry> {
        self.status_log.last()
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

    #[test]
    fn status_log_keeps_latest_entries_only() {
        let mut state = AppState::new("fake", PermissionMode::Suggest);

        for index in 0..105 {
            state.push_status_message(format!("status {index}"));
        }

        assert_eq!(state.status_log.len(), AppState::STATUS_LOG_LIMIT);
        assert_eq!(
            state
                .latest_status_message()
                .map(|entry| entry.message.as_str()),
            Some("status 104")
        );
        assert_eq!(
            state.status_log.first().map(|entry| entry.message.as_str()),
            Some("status 5")
        );
    }
}
