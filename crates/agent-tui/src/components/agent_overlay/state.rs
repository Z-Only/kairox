//! State types and behaviour for [`AgentOverlay`].
//!
//! The overlay tracks list/editor mode plus an in-progress agent draft, and
//! exposes high-level helpers used by the [`Component`](crate::components::Component)
//! implementation in [`super`] and the rendering helpers in
//! [`super::render`].

use agent_core::facade::{AgentSettingsScope, AgentSettingsView};
use ratatui::widgets::ListState;

use super::types::{AgentDraft, AgentEditorField, AgentOverlayMode, EDITOR_FIELDS};
use crate::components::AgentOverlaySnapshot;

pub struct AgentOverlay {
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) agents: Vec<AgentSettingsView>,
    pub(super) list_state: ListState,
    pub(super) mode: AgentOverlayMode,
    pub(super) draft: AgentDraft,
    pub(super) editor_field_index: usize,
}

impl Default for AgentOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            agents: Vec::new(),
            list_state: ListState::default(),
            mode: AgentOverlayMode::List,
            draft: AgentDraft::new(AgentSettingsScope::User),
            editor_field_index: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: AgentOverlaySnapshot) {
        let selected = if snapshot.agents.is_empty() {
            None
        } else {
            Some(
                self.list_state
                    .selected()
                    .unwrap_or(0)
                    .min(snapshot.agents.len().saturating_sub(1)),
            )
        };
        self.agents = snapshot.agents;
        self.list_state.select(selected);
        self.mode = AgentOverlayMode::List;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.agents.clear();
        self.list_state.select(None);
        self.mode = AgentOverlayMode::List;
        self.draft = AgentDraft::new(AgentSettingsScope::User);
        self.editor_field_index = 0;
    }

    #[allow(dead_code)]
    pub fn agents(&self) -> &[AgentSettingsView] {
        &self.agents
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub(super) fn selected_agent(&self) -> Option<&AgentSettingsView> {
        self.list_state
            .selected()
            .and_then(|index| self.agents.get(index))
    }

    pub(super) fn current_editor_field(&self) -> AgentEditorField {
        EDITOR_FIELDS[self.editor_field_index]
    }
}
