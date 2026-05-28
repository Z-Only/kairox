//! State types for the help overlay.

use crate::components::{FocusTarget, HelpOverlaySnapshot};

#[derive(Debug, Clone)]
pub struct HelpOverlay {
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) snapshot: HelpOverlaySnapshot,
}

impl Default for HelpOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl HelpOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            snapshot: HelpOverlaySnapshot {
                focus: FocusTarget::Chat,
            },
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: HelpOverlaySnapshot) {
        self.visible = true;
        self.snapshot = snapshot;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Shortcut {
    pub key: &'static str,
    pub label: &'static str,
}
