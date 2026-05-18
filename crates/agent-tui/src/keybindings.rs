//! Keybinding resolver for the interactive TUI.
//!
//! Keys are resolved in priority order:
//!
//! | Layer | Scope      | Examples                           |
//! |-------|------------|-------------------------------------|
//! | L2    | Alt        | Alt+s/t/e/p/n/q/1/2/3             |
//! | L3    | Ctrl       | Ctrl+C, Ctrl+L, Ctrl+Enter         |
//! | L4    | Function   | F1, F2, F5                         |
//! | L1    | Instant    | Enter, Esc, Tab, Y/N/D, etc.       |
//!
//! L2 and L3 are **global** — they fire regardless of focus.
//! L4 depends on focus context.
//! L1 depends on focus, input mode, and permission-pending state.

mod action;
mod density;
mod resolver;

pub use action::KeyAction;
pub use density::TraceDensity;
pub use resolver::{resolve_key, resolve_paste};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests;
