use crate::app_state::{InputMode, InputState};
use crate::components::{Command, CrossPanelEffect, EventContext, QueueAction, QueuedMessage};
use crate::keybindings::KeyAction;
use agent_core::AttachmentInfo;
use std::path::{Path, PathBuf};

use super::{is_session_busy, ChatPanel};

impl ChatPanel {
    /// Central input-handling method. Resolves a [`KeyAction`] produced by
    /// the keybinding layer and returns (cross-panel effects, commands).
    pub fn apply_key_action(
        &mut self,
        action: KeyAction,
        ctx: &EventContext,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match action {
            KeyAction::SendInput
                if !self.input_content.is_empty() || !self.pending_attachments.is_empty() =>
            {
                let trimmed = self.input_content.trim();
                if trimmed == ":compact" {
                    self.clear_input();
                    if let Some(session_id) = ctx.current_session_id {
                        commands.push(Command::CompactSession {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                        });
                    }
                } else if let Some(alias) = trimmed
                    .strip_prefix(":model ")
                    .map(str::trim)
                    .filter(|a| !a.is_empty())
                {
                    let alias = alias.to_string();
                    self.clear_input();
                    if let Some(session_id) = ctx.current_session_id {
                        commands.push(Command::SwitchModel {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            alias,
                            reasoning_effort: None,
                        });
                    }
                } else if trimmed == ":skills" {
                    self.clear_input();
                    commands.push(Command::ListSkills);
                } else if trimmed == ":instructions" {
                    self.clear_input();
                    commands.push(Command::OpenInstructionsOverlay);
                } else if trimmed == ":plugins" {
                    self.clear_input();
                    commands.push(Command::OpenPluginsOverlay);
                } else if trimmed == ":project draft" {
                    self.clear_input();
                    if let Some(project_id) = active_project_id(ctx) {
                        commands.push(Command::CreateProjectDraftSession { project_id });
                    }
                } else if let Some(branch_name) = trimmed
                    .strip_prefix(":project worktree ")
                    .map(str::trim)
                    .filter(|branch_name| !branch_name.is_empty())
                    .map(str::to_string)
                {
                    self.clear_input();
                    if let Some(project_id) = active_project_id(ctx) {
                        commands.push(Command::CreateProjectWorktreeSession {
                            project_id,
                            branch_name,
                        });
                    }
                } else if let Some(path) = trimmed
                    .strip_prefix(":attach ")
                    .map(str::trim)
                    .filter(|path| !path.is_empty())
                {
                    if let Some(attachment) = attachment_from_path(path) {
                        self.push_pending_attachment(attachment);
                    }
                    self.clear_input();
                } else if trimmed == ":detach" {
                    self.pending_attachments.clear();
                    self.clear_input();
                } else if let Some(target) = trimmed
                    .strip_prefix(":detach ")
                    .map(str::trim)
                    .filter(|target| !target.is_empty())
                    .map(str::to_string)
                {
                    self.detach_attachment(&target);
                    self.clear_input();
                } else if let Some(skill_id) = trimmed
                    .strip_prefix(":skill show ")
                    .map(str::trim)
                    .filter(|skill_id| !skill_id.is_empty())
                    .map(str::to_string)
                {
                    self.clear_input();
                    commands.push(Command::ShowSkill { skill_id });
                } else if let Some(skill_id) = trimmed
                    .strip_prefix(":skill activate ")
                    .map(str::trim)
                    .filter(|skill_id| !skill_id.is_empty())
                    .map(str::to_string)
                {
                    self.clear_input();
                    if let Some(session_id) = ctx.current_session_id {
                        commands.push(Command::ActivateSkill {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            skill_id,
                        });
                    }
                } else if let Some(skill_id) = trimmed
                    .strip_prefix(":skill deactivate ")
                    .map(str::trim)
                    .filter(|skill_id| !skill_id.is_empty())
                    .map(str::to_string)
                {
                    self.clear_input();
                    if let Some(session_id) = ctx.current_session_id {
                        commands.push(Command::DeactivateSkill {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            skill_id,
                        });
                    }
                } else if trimmed == ":skill catalog" || trimmed.starts_with(":skill catalog ") {
                    let keyword = trimmed
                        .strip_prefix(":skill catalog")
                        .map(str::trim)
                        .filter(|keyword| !keyword.is_empty())
                        .map(str::to_string);
                    self.clear_input();
                    commands.push(Command::ListSkillCatalog { keyword });
                } else if let Some(package) = trimmed
                    .strip_prefix(":skill install ")
                    .map(str::trim)
                    .filter(|package| !package.is_empty())
                    .map(str::to_string)
                {
                    self.clear_input();
                    if let Some(source) = package
                        .strip_prefix("github ")
                        .map(str::trim)
                        .filter(|source| !source.is_empty())
                        .map(str::to_string)
                    {
                        commands.push(Command::InstallGithubSkill {
                            request: agent_core::facade::InstallGithubSkillRequest {
                                source,
                                target: agent_core::facade::SkillInstallTarget::User,
                            },
                        });
                    } else {
                        commands.push(Command::InstallRemoteSkill {
                            request: agent_core::facade::InstallRemoteSkillRequest {
                                package: package.clone(),
                                source: package,
                                target: agent_core::facade::SkillInstallTarget::User,
                                package_url: None,
                            },
                        });
                    }
                } else if let Some(skill_id) = trimmed
                    .strip_prefix(":skill update ")
                    .map(str::trim)
                    .filter(|skill_id| !skill_id.is_empty())
                    .map(str::to_string)
                {
                    self.clear_input();
                    commands.push(Command::UpdateSkillSettings { skill_id });
                } else if let Some(skill_id) = trimmed
                    .strip_prefix(":skill delete ")
                    .map(str::trim)
                    .filter(|skill_id| !skill_id.is_empty())
                    .map(str::to_string)
                {
                    self.clear_input();
                    commands.push(Command::DeleteSkillSettings { skill_id });
                } else if let Some(queue_action) = parse_queue_action(trimmed) {
                    self.clear_input();
                    if let Some(command) = self.apply_queue_action(queue_action, ctx) {
                        commands.push(command);
                    }
                } else {
                    if !self.input_content.is_empty() {
                        self.input_history.push(self.input_content.clone());
                    }
                    let content = std::mem::take(&mut self.input_content);
                    let attachments = std::mem::take(&mut self.pending_attachments);
                    self.input_cursor = 0;
                    self.input_history_index = None;

                    if is_session_busy(ctx) {
                        // Session is busy — hold the message locally and drain
                        // it when the session returns to idle.
                        self.message_queue.push(QueuedMessage {
                            content,
                            attachments,
                        });
                    } else if let Some(session_id) = ctx.current_session_id {
                        commands.push(Command::SendMessage {
                            workspace_id: ctx.workspace_id.clone(),
                            session_id: session_id.clone(),
                            content,
                            attachments,
                        });
                    }
                }
            }
            KeyAction::InputCharacter(c) => {
                self.input_content.insert(self.input_cursor, c);
                self.input_cursor += c.len_utf8();
            }
            KeyAction::InputBackspace if self.input_cursor > 0 => {
                let prev = prev_char_boundary(&self.input_content, self.input_cursor);
                self.input_content.drain(prev..self.input_cursor);
                self.input_cursor = prev;
            }
            KeyAction::InputNewline if self.input_mode == InputMode::MultiLine => {
                self.input_content.insert(self.input_cursor, '\n');
                self.input_cursor += 1;
            }
            KeyAction::ToggleInputMode => {
                self.input_mode = match self.input_mode {
                    InputMode::SingleLine => InputMode::MultiLine,
                    InputMode::MultiLine => InputMode::SingleLine,
                };
            }
            KeyAction::InputHistoryUp if !self.input_history.is_empty() => {
                let idx = match self.input_history_index {
                    Some(i) if i > 0 => i - 1,
                    Some(i) => i,
                    None => self.input_history.len() - 1,
                };
                self.input_history_index = Some(idx);
                self.input_content = self.input_history[idx].clone();
                self.input_cursor = self.input_content.len();
            }
            KeyAction::InputHistoryDown => {
                if let Some(i) = self.input_history_index {
                    if i + 1 < self.input_history.len() {
                        let next = i + 1;
                        self.input_history_index = Some(next);
                        self.input_content = self.input_history[next].clone();
                    } else {
                        self.input_history_index = None;
                        self.input_content.clear();
                    }
                    self.input_cursor = self.input_content.len();
                }
            }
            KeyAction::AllowPermission => {
                if let InputState::PermissionWait { request_id, .. } = &self.input_state {
                    let rid = request_id.clone();
                    self.input_state = InputState::Normal;
                    commands.push(Command::DecidePermission {
                        request_id: rid,
                        approved: true,
                    });
                    effects.push(CrossPanelEffect::DismissPermissionPrompt);
                }
            }
            KeyAction::DenyPermission | KeyAction::DenyAllPermission => {
                if let InputState::PermissionWait { request_id, .. } = &self.input_state {
                    let rid = request_id.clone();
                    self.input_state = InputState::Normal;
                    commands.push(Command::DecidePermission {
                        request_id: rid,
                        approved: false,
                    });
                    effects.push(CrossPanelEffect::DismissPermissionPrompt);
                }
            }
            KeyAction::Escape
                if self.input_mode == InputMode::MultiLine && self.input_content.is_empty() =>
            {
                self.input_mode = InputMode::SingleLine;
            }
            KeyAction::InputPaste(text) => {
                for c in text.chars() {
                    self.input_content.insert(self.input_cursor, c);
                    self.input_cursor += c.len_utf8();
                }
            }
            KeyAction::ApplyQueueAction(action) => {
                if let Some(command) = self.apply_queue_action(action, ctx) {
                    commands.push(command);
                }
            }
            _ => {}
        }

        (effects, commands)
    }

    fn clear_input(&mut self) {
        self.input_content.clear();
        self.input_cursor = 0;
        self.input_history_index = None;
    }

    fn push_pending_attachment(&mut self, attachment: AttachmentInfo) {
        if self
            .pending_attachments
            .iter()
            .any(|existing| existing.path == attachment.path)
        {
            return;
        }
        self.pending_attachments.push(attachment);
    }

    fn detach_attachment(&mut self, target: &str) {
        let target_path = normalize_attachment_path(target);
        self.pending_attachments.retain(|attachment| {
            attachment.name != target
                && target_path
                    .as_ref()
                    .is_none_or(|path| attachment.path != path.display().to_string())
        });
    }

    pub(crate) fn apply_queue_action(
        &mut self,
        action: QueueAction,
        ctx: &EventContext,
    ) -> Option<Command> {
        match action {
            QueueAction::View => None,
            QueueAction::SelectPrevious => {
                self.select_previous_queued_message();
                None
            }
            QueueAction::SelectNext => {
                self.select_next_queued_message();
                None
            }
            QueueAction::MoveSelectedUp => {
                self.move_selected_queued_message_up();
                None
            }
            QueueAction::MoveSelectedDown => {
                self.move_selected_queued_message_down();
                None
            }
            QueueAction::RestoreSelectedForEdit => {
                self.restore_selected_queued_message_for_edit();
                None
            }
            QueueAction::DeleteSelected => {
                self.delete_selected_queued_message();
                None
            }
            QueueAction::SendSelectedNow => {
                let queue_index = self.selected_queue_index()?;
                let session_id = ctx.current_session_id.as_ref()?.clone();
                Some(Command::SendQueuedMessageNow {
                    workspace_id: ctx.workspace_id.clone(),
                    session_id,
                    queue_index,
                })
            }
        }
    }
}

fn parse_queue_action(trimmed: &str) -> Option<QueueAction> {
    let rest = trimmed.strip_prefix(":queue").map(str::trim)?;
    match rest {
        "" | "list" | "show" => Some(QueueAction::View),
        "prev" | "previous" | "up-select" => Some(QueueAction::SelectPrevious),
        "next" => Some(QueueAction::SelectNext),
        "up" | "move-up" => Some(QueueAction::MoveSelectedUp),
        "down" | "move-down" => Some(QueueAction::MoveSelectedDown),
        "edit" | "restore" => Some(QueueAction::RestoreSelectedForEdit),
        "delete" | "remove" => Some(QueueAction::DeleteSelected),
        "send" | "send-now" | "now" => Some(QueueAction::SendSelectedNow),
        _ => None,
    }
}

fn active_project_id(ctx: &EventContext) -> Option<agent_core::ProjectId> {
    let session_id = ctx.current_session_id.as_ref()?;
    ctx.sessions
        .iter()
        .find(|session| &session.id == session_id)
        .and_then(|session| session.project_id.clone())
}

fn prev_char_boundary(s: &str, pos: usize) -> usize {
    if pos == 0 {
        return 0;
    }
    let mut i = pos - 1;
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}

fn attachment_from_path(path: &str) -> Option<AttachmentInfo> {
    let normalized = normalize_attachment_path(path)?;
    let name = normalized
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path)
        .to_string();
    let mime_type = mime_type_for_path(&normalized).to_string();
    Some(AttachmentInfo {
        path: normalized.display().to_string(),
        name,
        mime_type,
    })
}

fn normalize_attachment_path(path: &str) -> Option<PathBuf> {
    let stripped = strip_wrapping_quotes(path.trim());
    if stripped.is_empty() {
        return None;
    }
    let expanded = expand_tilde(stripped);
    Some(std::fs::canonicalize(&expanded).unwrap_or(expanded))
}

fn strip_wrapping_quotes(value: &str) -> &str {
    if value.len() >= 2 {
        let first = value.as_bytes()[0];
        let last = value.as_bytes()[value.len() - 1];
        if (first == b'\'' && last == b'\'') || (first == b'"' && last == b'"') {
            return &value[1..value.len() - 1];
        }
    }
    value
}

fn expand_tilde(path: &str) -> PathBuf {
    if path == "~" {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(path)
}

fn mime_type_for_path(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("bmp") => "image/bmp",
        Some("css") => "text/css",
        Some("csv") => "text/csv",
        Some("gif") => "image/gif",
        Some("htm") | Some("html") => "text/html",
        Some("jpeg") | Some("jpg") => "image/jpeg",
        Some("js") | Some("mjs") | Some("ts") | Some("tsx") => "application/javascript",
        Some("json") => "application/json",
        Some("md") | Some("markdown") => "text/markdown",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("rs") => "text/x-rust",
        Some("sh") | Some("bash") | Some("zsh") => "application/x-sh",
        Some("svg") => "image/svg+xml",
        Some("toml") => "application/toml",
        Some("txt") | Some("text") => "text/plain",
        Some("xml") => "application/xml",
        Some("yaml") | Some("yml") => "application/x-yaml",
        Some("webp") => "image/webp",
        _ => "application/octet-stream",
    }
}
