//! State types and behaviour for [`HooksOverlay`].
//!
//! The overlay tracks tab/mode selection plus an in-progress hook draft, and
//! exposes high-level helpers used by the [`Component`](crate::components::Component)
//! implementation in [`super::mod`](super) and the rendering helpers in
//! [`super::render`](super::render).
//!
//! Key-event handling lives in [`super::keys`].

#[cfg(test)]
use agent_core::facade::HookSettingsInput;
use agent_core::facade::{HookSettingsView, HookTemplateView, HooksSettingsView};
use agent_core::ConfigScope;
use ratatui::widgets::ListState;

use super::types::{HookDraft, HookEditorField, HooksMode, HooksTab, EDITOR_FIELDS};

pub struct HooksOverlay {
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) tab: HooksTab,
    pub(super) mode: HooksMode,
    pub(super) user: Vec<HookSettingsView>,
    pub(super) project: Vec<HookSettingsView>,
    pub(super) templates: Vec<HookTemplateView>,
    pub(super) user_config_path: String,
    pub(super) project_config_path: Option<String>,
    pub(super) user_state: ListState,
    pub(super) project_state: ListState,
    pub(super) template_state: ListState,
    pub(super) draft: HookDraft,
    pub(super) editor_field_index: usize,
}

impl Default for HooksOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl HooksOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            tab: HooksTab::User,
            mode: HooksMode::List,
            user: Vec::new(),
            project: Vec::new(),
            templates: Vec::new(),
            user_config_path: String::new(),
            project_config_path: None,
            user_state: ListState::default(),
            project_state: ListState::default(),
            template_state: ListState::default(),
            draft: HookDraft::new(ConfigScope::User),
            editor_field_index: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, view: HooksSettingsView) {
        self.user = view.user;
        self.project = view.project;
        self.templates = view.templates;
        self.user_config_path = view.user_config_path;
        self.project_config_path = view.project_config_path;
        self.mode = HooksMode::List;
        self.visible = true;
        self.ensure_selection();
    }

    pub fn set_active_scope(&mut self, scope: ConfigScope) {
        self.tab = match scope {
            ConfigScope::Project => HooksTab::Project,
            _ => HooksTab::User,
        };
        self.ensure_selection();
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.mode = HooksMode::List;
        self.tab = HooksTab::User;
        self.user.clear();
        self.project.clear();
        self.templates.clear();
        self.user_state.select(None);
        self.project_state.select(None);
        self.template_state.select(None);
        self.draft = HookDraft::new(ConfigScope::User);
        self.editor_field_index = 0;
    }

    #[allow(dead_code)]
    pub fn user_hooks(&self) -> &[HookSettingsView] {
        &self.user
    }

    #[allow(dead_code)]
    pub fn project_hooks(&self) -> &[HookSettingsView] {
        &self.project
    }

    pub(super) fn current_len(&self) -> usize {
        match self.tab {
            HooksTab::User => self.user.len(),
            HooksTab::Project => self.project.len(),
            HooksTab::Templates => self.templates.len(),
        }
    }

    pub(super) fn current_selected(&self) -> Option<usize> {
        match self.tab {
            HooksTab::User => self.user_state.selected(),
            HooksTab::Project => self.project_state.selected(),
            HooksTab::Templates => self.template_state.selected(),
        }
    }

    pub(super) fn current_state_mut(&mut self) -> &mut ListState {
        match self.tab {
            HooksTab::User => &mut self.user_state,
            HooksTab::Project => &mut self.project_state,
            HooksTab::Templates => &mut self.template_state,
        }
    }

    pub(super) fn selected_hook(&self) -> Option<&HookSettingsView> {
        let index = self.current_selected()?;
        match self.tab {
            HooksTab::User => self.user.get(index),
            HooksTab::Project => self.project.get(index),
            HooksTab::Templates => None,
        }
    }

    pub(super) fn selected_template(&self) -> Option<&HookTemplateView> {
        self.template_state
            .selected()
            .and_then(|index| self.templates.get(index))
    }

    pub(super) fn ensure_selection(&mut self) {
        for (len, state) in [
            (self.user.len(), &mut self.user_state),
            (self.project.len(), &mut self.project_state),
            (self.templates.len(), &mut self.template_state),
        ] {
            if len == 0 {
                state.select(None);
            } else {
                let selected = state.selected().unwrap_or(0).min(len.saturating_sub(1));
                state.select(Some(selected));
            }
        }
    }

    pub(super) fn current_editor_field(&self) -> HookEditorField {
        EDITOR_FIELDS[self.editor_field_index]
    }

    #[cfg(test)]
    pub(super) fn draft_for_test(&self) -> &HookDraft {
        &self.draft
    }

    #[cfg(test)]
    pub(super) fn replace_draft_for_test(&mut self, input: HookSettingsInput) {
        self.draft = HookDraft::from_input(input);
        self.mode = HooksMode::Editor;
        self.visible = true;
    }
}
