//! ChatPanel component — message display and text input for the interactive TUI.

mod input;
mod render;

pub use render::{
    format_attachment_labels, render_file_mention_palette, render_messages, render_queue_strip,
};

use agent_core::AttachmentInfo;
use ratatui::layout::Rect;
use ratatui::Frame;
use std::path::PathBuf;

use crate::app_state::{InputMode, InputState};
use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, FocusTarget, QueuedMessage, RiskLevel,
    SessionState,
};

// ---------------------------------------------------------------------------
// ChatPanel
// ---------------------------------------------------------------------------

/// The main chat panel: displays messages from a [`SessionProjection`] and
/// handles text input, history navigation, and permission decisions.
#[allow(dead_code)]
pub struct ChatPanel {
    focused: bool,
    pub input_content: String,
    pub input_cursor: usize,
    pub input_mode: InputMode,
    pub input_state: InputState,
    pub input_history: Vec<String>,
    /// `None` means we're at the "live" position (not browsing history).
    pub input_history_index: Option<usize>,
    pub scroll_offset: usize,
    /// Files attached to the next user message.
    pub pending_attachments: Vec<AttachmentInfo>,
    /// Messages typed while the session was busy. Drained in FIFO order when
    /// the session returns to idle (see `drain_queue`).
    pub message_queue: Vec<QueuedMessage>,
    /// Selected queued message for local queue controls.
    pub selected_queue_index: usize,
    workspace_root: Option<PathBuf>,
    workspace_files: Vec<String>,
    file_mentions: FileMentionState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct FileMentionState {
    active: bool,
    token_start: usize,
    token_end: usize,
    matches: Vec<String>,
    selected_index: usize,
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
            pending_attachments: Vec::new(),
            message_queue: Vec::new(),
            selected_queue_index: 0,
            workspace_root: None,
            workspace_files: Vec::new(),
            file_mentions: FileMentionState::default(),
        }
    }

    pub fn set_workspace_files(&mut self, root: impl Into<PathBuf>, mut files: Vec<String>) {
        files.sort();
        files.dedup();
        self.workspace_root = Some(root.into());
        self.workspace_files = files;
        self.refresh_file_mentions();
    }

    pub fn set_draft_text(&mut self, text: impl Into<String>) {
        self.input_content = text.into();
        self.input_cursor = self.input_content.len();
        self.input_history_index = None;
        self.refresh_file_mentions();
    }

    pub fn file_mentions_visible(&self) -> bool {
        self.file_mentions.active
    }

    pub fn file_mention_matches(&self) -> &[String] {
        &self.file_mentions.matches
    }

    pub fn selected_file_mention_index(&self) -> Option<usize> {
        if self.file_mentions.active && !self.file_mentions.matches.is_empty() {
            Some(
                self.file_mentions
                    .selected_index
                    .min(self.file_mentions.matches.len() - 1),
            )
        } else {
            None
        }
    }

    /// Drain all queued messages in FIFO order.
    pub fn drain_queue(&mut self) -> Vec<QueuedMessage> {
        self.selected_queue_index = 0;
        std::mem::take(&mut self.message_queue)
    }

    pub fn selected_queue_index(&self) -> Option<usize> {
        if self.message_queue.is_empty() {
            None
        } else {
            Some(self.selected_queue_index.min(self.message_queue.len() - 1))
        }
    }

    pub fn queued_message(&self, index: usize) -> Option<&QueuedMessage> {
        self.message_queue.get(index)
    }

    pub fn remove_queued_message(&mut self, index: usize) -> Option<QueuedMessage> {
        if index >= self.message_queue.len() {
            return None;
        }
        let removed = self.message_queue.remove(index);
        self.clamp_queue_selection();
        Some(removed)
    }

    pub fn select_previous_queued_message(&mut self) -> bool {
        if self.message_queue.is_empty() {
            self.selected_queue_index = 0;
            return false;
        }
        if self.selected_queue_index > 0 {
            self.selected_queue_index -= 1;
        }
        true
    }

    pub fn select_next_queued_message(&mut self) -> bool {
        if self.message_queue.is_empty() {
            self.selected_queue_index = 0;
            return false;
        }
        if self.selected_queue_index + 1 < self.message_queue.len() {
            self.selected_queue_index += 1;
        }
        true
    }

    pub fn move_selected_queued_message_up(&mut self) -> bool {
        let Some(index) = self.selected_queue_index() else {
            return false;
        };
        if index == 0 {
            return true;
        }
        self.message_queue.swap(index, index - 1);
        self.selected_queue_index = index - 1;
        true
    }

    pub fn move_selected_queued_message_down(&mut self) -> bool {
        let Some(index) = self.selected_queue_index() else {
            return false;
        };
        if index + 1 >= self.message_queue.len() {
            return true;
        }
        self.message_queue.swap(index, index + 1);
        self.selected_queue_index = index + 1;
        true
    }

    pub fn delete_selected_queued_message(&mut self) -> Option<QueuedMessage> {
        let index = self.selected_queue_index()?;
        self.remove_queued_message(index)
    }

    pub fn restore_selected_queued_message_for_edit(&mut self) -> bool {
        let Some(index) = self.selected_queue_index() else {
            return false;
        };
        let Some(queued) = self.remove_queued_message(index) else {
            return false;
        };
        self.input_content = queued.content;
        self.input_cursor = self.input_content.len();
        self.input_history_index = None;
        self.pending_attachments = queued.attachments;
        true
    }

    fn clamp_queue_selection(&mut self) {
        if self.message_queue.is_empty() {
            self.selected_queue_index = 0;
        } else if self.selected_queue_index >= self.message_queue.len() {
            self.selected_queue_index = self.message_queue.len() - 1;
        }
    }

    fn refresh_file_mentions(&mut self) {
        let Some((start, end, filter)) =
            file_mention_token_before_cursor(&self.input_content, self.input_cursor)
        else {
            self.file_mentions = FileMentionState::default();
            return;
        };

        let matches = matching_workspace_files(&self.workspace_files, &filter, 20);
        let selected_index = self
            .file_mentions
            .selected_index
            .min(matches.len().saturating_sub(1));
        self.file_mentions = FileMentionState {
            active: true,
            token_start: start,
            token_end: end,
            matches,
            selected_index,
        };
    }

    fn close_file_mentions(&mut self) {
        self.file_mentions = FileMentionState::default();
    }

    fn select_previous_file_mention(&mut self) {
        if !self.file_mentions.active || self.file_mentions.matches.is_empty() {
            return;
        }
        if self.file_mentions.selected_index > 0 {
            self.file_mentions.selected_index -= 1;
        }
    }

    fn select_next_file_mention(&mut self) {
        if !self.file_mentions.active || self.file_mentions.matches.is_empty() {
            return;
        }
        if self.file_mentions.selected_index + 1 < self.file_mentions.matches.len() {
            self.file_mentions.selected_index += 1;
        }
    }

    fn accept_selected_file_mention(&mut self) -> bool {
        if !self.file_mentions.active {
            return false;
        }
        let Some(relative_path) = self
            .file_mentions
            .matches
            .get(self.file_mentions.selected_index)
            .cloned()
        else {
            return false;
        };

        let replacement = format!("@{relative_path} ");
        self.input_content.replace_range(
            self.file_mentions.token_start..self.file_mentions.token_end,
            &replacement,
        );
        self.input_cursor = self.file_mentions.token_start + replacement.len();

        if let Some(root) = &self.workspace_root {
            if let Some(attachment) =
                input::attachment_from_path(&root.join(&relative_path).display().to_string())
            {
                self.push_pending_attachment(attachment);
            }
        }

        self.close_file_mentions();
        true
    }
}

fn file_mention_token_before_cursor(text: &str, cursor: usize) -> Option<(usize, usize, String)> {
    if cursor > text.len() || !text.is_char_boundary(cursor) {
        return None;
    }
    let before_cursor = &text[..cursor];
    let token_start = before_cursor
        .char_indices()
        .rev()
        .find(|(_, ch)| ch.is_whitespace())
        .map(|(idx, ch)| idx + ch.len_utf8())
        .unwrap_or(0);
    let token = &before_cursor[token_start..];
    let filter = token.strip_prefix('@')?;
    if filter.contains('@') {
        return None;
    }
    Some((token_start, cursor, filter.to_string()))
}

fn matching_workspace_files(files: &[String], filter: &str, limit: usize) -> Vec<String> {
    let query = filter.to_ascii_lowercase();
    files
        .iter()
        .filter(|path| query.is_empty() || fuzzy_match(path, &query))
        .take(limit)
        .cloned()
        .collect()
}

fn fuzzy_match(path: &str, query: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let mut query_chars = query.chars();
    let Some(mut current) = query_chars.next() else {
        return true;
    };
    for ch in lower.chars() {
        if ch == current {
            if let Some(next) = query_chars.next() {
                current = next;
            } else {
                return true;
            }
        }
    }
    false
}

/// Whether the current session is busy (running) — Enter typed in this state
/// must enqueue rather than send. Matches the GUI `isQueueing` semantics:
/// busy when the assistant is streaming tokens, a tool is running, or the
/// session is awaiting permission.
pub(crate) fn is_session_busy(ctx: &EventContext) -> bool {
    if !ctx.current_session.token_stream.is_empty() {
        return true;
    }
    let Some(sid) = ctx.current_session_id else {
        return false;
    };
    ctx.sessions
        .iter()
        .find(|s| s.id == *sid)
        .map(|s| {
            matches!(
                s.state,
                SessionState::Active | SessionState::AwaitingPermission
            )
        })
        .unwrap_or(false)
}

impl Default for ChatPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for ChatPanel {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &crossterm::event::Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let permission_pending = matches!(self.input_state, InputState::PermissionWait { .. });
        let action = crate::keybindings::resolve_key(
            match event {
                crossterm::event::Event::Key(ke) => *ke,
                _ => return (Vec::new(), Vec::new()),
            },
            FocusTarget::Chat,
            permission_pending,
            self.input_mode,
        );
        self.apply_key_action(action, ctx)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowPermissionPrompt(req) if req.risk_level == RiskLevel::Write => {
                // Only handle Write-level risks in ChatPanel.
                // Destructive risks are handled by PermissionModal.
                self.input_state = InputState::PermissionWait {
                    request_id: req.request_id.clone(),
                    pending_prompt: req.tool_preview.clone(),
                };
            }
            CrossPanelEffect::ShowPermissionPrompt(_) => {}
            CrossPanelEffect::DismissPermissionPrompt => {
                if matches!(self.input_state, InputState::PermissionWait { .. }) {
                    self.input_state = InputState::Normal;
                }
            }
            CrossPanelEffect::StartStreaming | CrossPanelEffect::StopStreaming => {
                // No-op for now; will be wired to RenderScheduler later.
            }
            CrossPanelEffect::PrefillChatInput(text) => {
                self.input_content = text.clone();
                self.input_cursor = text.len();
                self.input_history_index = None;
            }
            _ => {}
        }
    }

    /// Placeholder — the App layer handles ChatPanel rendering centrally
    /// via [`render_messages`].
    fn render(&self, _area: Rect, _frame: &mut Frame) {}

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
