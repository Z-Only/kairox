//! Monitor overlay state — the [`MonitorOverlay`] component data model.

use ratatui::widgets::ListState;

use super::types::MonitorEntry;
use crate::components::MonitorOverlaySnapshot;

pub struct MonitorOverlay {
    pub(super) visible: bool,
    pub(super) focused: bool,
    pub(super) monitors: Vec<MonitorEntry>,
    pub(super) list_state: ListState,
}

impl Default for MonitorOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl MonitorOverlay {
    pub fn new() -> Self {
        Self {
            visible: false,
            focused: false,
            monitors: Vec::new(),
            list_state: ListState::default(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: MonitorOverlaySnapshot) {
        self.monitors = snapshot.monitors;
        self.visible = true;
        self.ensure_selection();
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.monitors.clear();
        self.list_state.select(None);
    }

    #[allow(dead_code)]
    pub fn monitors(&self) -> &[MonitorEntry] {
        &self.monitors
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub(super) fn selected_monitor(&self) -> Option<&MonitorEntry> {
        self.list_state
            .selected()
            .and_then(|i| self.monitors.get(i))
    }

    pub(super) fn ensure_selection(&mut self) {
        if self.monitors.is_empty() {
            self.list_state.select(None);
        } else if self.list_state.selected().is_none() {
            self.list_state.select(Some(0));
        } else if let Some(i) = self.list_state.selected() {
            if i >= self.monitors.len() {
                self.list_state
                    .select(Some(self.monitors.len().saturating_sub(1)));
            }
        }
    }
}
