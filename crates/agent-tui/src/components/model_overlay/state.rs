//! Model overlay state — query helpers and the main `ModelOverlay` struct.
//!
//! Data types (enums, draft buffer) live in [`super::types`]; interactive
//! key-handling logic lives in [`super::keys`]. Rendering lives in
//! [`super::render`].

use std::collections::BTreeMap;

use ratatui::widgets::ListState;

use super::types::{OverlayFocus, OverlayMode, ProfileDraft, ProfileEditorField, PROFILE_EDITOR_FIELDS};
use crate::components::{ModelOverlaySnapshot, ModelProfileEntry, ModelProfileTestResult};

/// Effort presets exposed for reasoning-capable profiles. Mirrors the GUI's
/// `DEFAULT_REASONING_EFFORTS` constant in `apps/agent-gui/src/stores/session.ts`.
pub const REASONING_EFFORTS: [&str; 4] = ["low", "middle", "high", "xhigh"];

pub struct ModelOverlay {
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) profiles: Vec<ModelProfileEntry>,
    pub(super) current_alias: Option<String>,
    pub(super) current_effort: Option<String>,
    pub(super) list_state: ListState,
    pub(super) effort_state: ListState,
    pub(super) overlay_focus: OverlayFocus,
    pub(super) mode: OverlayMode,
    pub(super) draft: ProfileDraft,
    pub(super) editor_field_index: usize,
    pub(super) test_results: BTreeMap<String, ModelProfileTestResult>,
}

impl Default for ModelOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            profiles: Vec::new(),
            current_alias: None,
            current_effort: None,
            list_state: ListState::default(),
            effort_state: ListState::default(),
            overlay_focus: OverlayFocus::ProfileList,
            mode: OverlayMode::List,
            draft: ProfileDraft::new(),
            editor_field_index: 0,
            test_results: BTreeMap::new(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: ModelOverlaySnapshot) {
        // Default selection: the current alias if it exists in the list, else 0.
        let select = if snapshot.profiles.is_empty() {
            None
        } else {
            snapshot
                .current_alias
                .as_ref()
                .and_then(|a| snapshot.profiles.iter().position(|p| &p.alias == a))
                .or(Some(0))
        };
        self.list_state.select(select);

        // Effort selection mirrors current_effort when present and the selected
        // profile supports reasoning; else default to "low" so the picker has
        // a visible cursor.
        self.current_alias = snapshot.current_alias;
        self.current_effort = snapshot.current_effort;
        self.profiles = snapshot.profiles;
        let initial_effort = self
            .current_effort
            .as_deref()
            .and_then(|e| REASONING_EFFORTS.iter().position(|x| *x == e))
            .unwrap_or(0);
        self.effort_state.select(Some(initial_effort));
        self.overlay_focus = OverlayFocus::ProfileList;
        self.mode = OverlayMode::List;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.profiles.clear();
        self.list_state.select(None);
        self.effort_state.select(None);
        self.current_alias = None;
        self.current_effort = None;
        self.overlay_focus = OverlayFocus::ProfileList;
        self.mode = OverlayMode::List;
        self.draft = ProfileDraft::new();
        self.editor_field_index = 0;
        self.test_results.clear();
    }

    #[allow(dead_code)]
    pub fn profiles(&self) -> &[ModelProfileEntry] {
        &self.profiles
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn selected_profile(&self) -> Option<&ModelProfileEntry> {
        self.list_state
            .selected()
            .and_then(|i| self.profiles.get(i))
    }

    pub(super) fn current_editor_field(&self) -> ProfileEditorField {
        PROFILE_EDITOR_FIELDS[self.editor_field_index]
    }

    /// `true` when the selected profile is reasoning-capable, so the effort
    /// picker should be rendered.
    pub fn shows_effort_picker(&self) -> bool {
        self.selected_profile()
            .map(|p| p.enabled && p.supports_reasoning)
            .unwrap_or(false)
    }

    /// Currently highlighted effort string (only meaningful when the selected
    /// profile supports reasoning).
    pub fn selected_effort(&self) -> Option<&'static str> {
        if !self.shows_effort_picker() {
            return None;
        }
        self.effort_state
            .selected()
            .and_then(|i| REASONING_EFFORTS.get(i).copied())
    }

    /// Available effort options for the selected profile. Empty for
    /// non-reasoning models.
    #[allow(dead_code)]
    pub fn effort_options(&self) -> &'static [&'static str] {
        if self.shows_effort_picker() {
            &REASONING_EFFORTS
        } else {
            &[]
        }
    }

    pub(super) fn set_test_result(&mut self, result: ModelProfileTestResult) {
        self.test_results.insert(result.alias.clone(), result);
    }
}

pub(super) fn format_optional<T: ToString>(value: Option<T>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

pub(super) fn parse_optional<T: std::str::FromStr>(value: &str) -> Option<T> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse().ok()
    }
}

pub(super) fn trim_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
