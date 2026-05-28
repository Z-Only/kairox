//! Data types for the help overlay — the `Shortcut` struct used by
//! rendering helpers to describe a keyboard shortcut entry.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Shortcut {
    pub key: &'static str,
    pub label: &'static str,
}
