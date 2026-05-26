//! Session/composer/queue input. Builds the shared [`EventContext`] and
//! forwards key actions or queue actions to the chat component, then captures
//! any draft-save side effect when the input buffer changes.

use crate::components::{Command, CrossPanelEffect, EventContext};
use crate::keybindings::KeyAction;

use crate::app::App;

impl App {
    pub(super) fn apply_chat_action(
        &mut self,
        action: KeyAction,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let draft_before = self.chat.input_content.clone();
        let focus = self.state.focus_manager.current();
        let projects = self.state.projects.clone();
        let sessions = self.state.sessions.clone();
        let model_profile = self.state.model_profile.clone();
        let sidebar_left = self.state.sidebar_left_visible;
        let sidebar_right = self.state.sidebar_right_visible;
        let ctx = EventContext {
            focus,
            current_session: &self.state.current_session,
            projects: &projects,
            sessions: &sessions,
            model_profile: &model_profile,
            sidebar_left_visible: sidebar_left,
            sidebar_right_visible: sidebar_right,
            workspace_id: &self.workspace_id,
            current_session_id: &self.current_session_id,
        };
        let (effects, mut commands) = self.chat.apply_key_action(action, &ctx);
        if self.chat.input_content != draft_before {
            if let Some(command) = self.current_draft_save_command() {
                commands.push(command);
            }
        }
        (effects, commands)
    }

    pub fn apply_queue_action(&mut self, action: crate::components::QueueAction) -> Vec<Command> {
        let draft_before = self.chat.input_content.clone();
        let focus = self.state.focus_manager.current();
        let projects = self.state.projects.clone();
        let sessions = self.state.sessions.clone();
        let model_profile = self.state.model_profile.clone();
        let sidebar_left = self.state.sidebar_left_visible;
        let sidebar_right = self.state.sidebar_right_visible;
        let ctx = EventContext {
            focus,
            current_session: &self.state.current_session,
            projects: &projects,
            sessions: &sessions,
            model_profile: &model_profile,
            sidebar_left_visible: sidebar_left,
            sidebar_right_visible: sidebar_right,
            workspace_id: &self.workspace_id,
            current_session_id: &self.current_session_id,
        };
        let command = self.chat.apply_queue_action(action, &ctx);
        self.state.render_scheduler.mark_dirty();
        let mut commands: Vec<Command> = command.into_iter().collect();
        if self.chat.input_content != draft_before {
            if let Some(command) = self.current_draft_save_command() {
                commands.push(command);
            }
        }
        commands
    }
}
