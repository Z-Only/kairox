//! Stack-based focus management for the TUI.

use crate::components::FocusTarget;

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
                | FocusTarget::MonitorOverlay
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
            | FocusTarget::MonitorOverlay
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
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "focus_tests.rs"]
mod tests;
