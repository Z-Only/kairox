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

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app_state::InputMode;
use crate::components::FocusTarget;

// ---------------------------------------------------------------------------
// KeyAction
// ---------------------------------------------------------------------------

/// All possible actions produced by resolving a key press.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyAction {
    // -- L1 Instant --------------------------------------------------------
    SendInput,
    Escape,
    FocusCycleNext,
    AllowPermission,
    DenyPermission,
    DenyAllPermission,
    ContextMenu,

    // -- L2 Alt ------------------------------------------------------------
    ToggleSessionsSidebar,
    ToggleTraceSidebar,
    ToggleInputMode,
    OpenProfileSelector,
    NewSession,
    Quit,
    FocusChat,
    FocusSessions,
    FocusTrace,

    // -- L3 Ctrl -----------------------------------------------------------
    InterruptOrQuit,
    Redraw,

    // -- L4 Function -------------------------------------------------------
    Help,
    RenameSession,
    ToggleTraceDensity,

    // -- Input -------------------------------------------------------------
    InputCharacter(char),
    InputBackspace,
    InputDelete,
    InputNewline,
    InputHistoryUp,
    InputHistoryDown,
    InputPaste(String),

    // -- Navigation --------------------------------------------------------
    ScrollUp,
    ScrollDown,
    SelectSession,

    /// Key was not bound in the current context.
    Unhandled,
}

// ---------------------------------------------------------------------------
// TraceDensity
// ---------------------------------------------------------------------------

/// Controls how much detail the trace panel shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TraceDensity {
    /// One-line summary per tool call.
    #[default]
    Summary,
    /// Expand tool call arguments and results.
    Expanded,
    /// Show every event in the stream (including heartbeats).
    FullEventStream,
    TaskGraph,
}

impl TraceDensity {
    /// Cycle to the next density level.
    pub fn next(self) -> Self {
        match self {
            Self::Summary => Self::Expanded,
            Self::Expanded => Self::FullEventStream,
            Self::FullEventStream => Self::TaskGraph,
            Self::TaskGraph => Self::Summary,
        }
    }
}

// ---------------------------------------------------------------------------
// Resolve
// ---------------------------------------------------------------------------

/// Resolve a `KeyEvent` into a [`KeyAction`].
///
/// The `permission_pending` flag is true when a permission prompt is
/// currently shown; in that state Y/N/D (and Esc) map to permission
/// decisions instead of their normal meanings.
pub fn resolve_key(
    key: KeyEvent,
    focus: FocusTarget,
    permission_pending: bool,
    input_mode: InputMode,
) -> KeyAction {
    let code = key.code;
    let mods = key.modifiers;

    // -----------------------------------------------------------------------
    // L2 — Alt (global, fires regardless of focus)
    // -----------------------------------------------------------------------
    if mods.contains(KeyModifiers::ALT) {
        return match code {
            KeyCode::Char('s') => KeyAction::ToggleSessionsSidebar,
            KeyCode::Char('t') => KeyAction::ToggleTraceSidebar,
            KeyCode::Char('e') => KeyAction::ToggleInputMode,
            KeyCode::Char('p') => KeyAction::OpenProfileSelector,
            KeyCode::Char('n') => KeyAction::NewSession,
            KeyCode::Char('q') => KeyAction::Quit,
            KeyCode::Char('1') => KeyAction::FocusChat,
            KeyCode::Char('2') => KeyAction::FocusSessions,
            KeyCode::Char('3') => KeyAction::FocusTrace,
            _ => KeyAction::Unhandled,
        };
    }

    // -----------------------------------------------------------------------
    // L3 — Ctrl (global)
    // -----------------------------------------------------------------------
    if mods.contains(KeyModifiers::CONTROL) {
        return match code {
            KeyCode::Char('c') => KeyAction::InterruptOrQuit,
            KeyCode::Char('l') => KeyAction::Redraw,
            KeyCode::Enter => KeyAction::SendInput,
            _ => KeyAction::Unhandled,
        };
    }

    // -----------------------------------------------------------------------
    // L4 — Function keys (focus-dependent)
    // -----------------------------------------------------------------------
    match code {
        KeyCode::F(1) => return KeyAction::Help,
        KeyCode::F(2) if focus == FocusTarget::Sessions => return KeyAction::RenameSession,
        KeyCode::F(5) if focus == FocusTarget::Trace => return KeyAction::ToggleTraceDensity,
        _ => {}
    }

    // -----------------------------------------------------------------------
    // L1 — Instant (focus / input-mode / permission-dependent)
    // -----------------------------------------------------------------------

    // When a permission prompt is pending, intercept Y/N/D and Esc first.
    if permission_pending {
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => return KeyAction::AllowPermission,
            KeyCode::Char('n') | KeyCode::Char('N') => return KeyAction::DenyPermission,
            KeyCode::Char('d') | KeyCode::Char('D') => return KeyAction::DenyAllPermission,
            KeyCode::Esc => return KeyAction::DenyPermission,
            _ => {}
        }
    }

    match code {
        KeyCode::Esc => KeyAction::Escape,

        KeyCode::Tab => KeyAction::FocusCycleNext,

        KeyCode::Enter => {
            if focus == FocusTarget::Chat {
                match input_mode {
                    InputMode::SingleLine => KeyAction::SendInput,
                    InputMode::MultiLine => KeyAction::InputNewline,
                }
            } else if focus == FocusTarget::Sessions {
                KeyAction::SelectSession
            } else {
                KeyAction::FocusCycleNext
            }
        }

        KeyCode::Up => {
            if focus == FocusTarget::Chat {
                KeyAction::InputHistoryUp
            } else {
                KeyAction::ScrollUp
            }
        }

        KeyCode::Down => {
            if focus == FocusTarget::Chat {
                KeyAction::InputHistoryDown
            } else {
                KeyAction::ScrollDown
            }
        }

        KeyCode::Backspace => KeyAction::InputBackspace,
        KeyCode::Delete => KeyAction::InputDelete,

        KeyCode::Char('x') => KeyAction::ContextMenu,

        KeyCode::Char(c) => KeyAction::InputCharacter(c),
        _ => KeyAction::Unhandled,
    }
}

/// Resolve a paste event into [`KeyAction::InputPaste`].
///
/// Paste events (from `crossterm::event::Event::Paste`) are handled at the
/// App level rather than inside the regular key resolver because the app
/// needs to decide whether to auto-upgrade `InputMode` to `MultiLine` when
/// the pasted text contains newlines.
pub fn resolve_paste(text: String) -> KeyAction {
    KeyAction::InputPaste(text)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a `KeyEvent` with the given code and no modifiers.
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Helper: build a `KeyEvent` with the given code and Alt modifier.
    fn alt_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::ALT)
    }

    /// Helper: build a `KeyEvent` with the given code and Ctrl modifier.
    fn ctrl_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    // -- L2 Alt keys resolve globally --------------------------------------

    #[test]
    fn l2_alt_keys_resolve_globally() {
        let focus = FocusTarget::Chat;
        let no_pending = false;
        let single = InputMode::SingleLine;

        assert_eq!(
            resolve_key(alt_key(KeyCode::Char('s')), focus, no_pending, single),
            KeyAction::ToggleSessionsSidebar
        );
        assert_eq!(
            resolve_key(alt_key(KeyCode::Char('t')), focus, no_pending, single),
            KeyAction::ToggleTraceSidebar
        );
        assert_eq!(
            resolve_key(alt_key(KeyCode::Char('e')), focus, no_pending, single),
            KeyAction::ToggleInputMode
        );
        assert_eq!(
            resolve_key(alt_key(KeyCode::Char('p')), focus, no_pending, single),
            KeyAction::OpenProfileSelector
        );
        assert_eq!(
            resolve_key(alt_key(KeyCode::Char('n')), focus, no_pending, single),
            KeyAction::NewSession
        );
        assert_eq!(
            resolve_key(alt_key(KeyCode::Char('q')), focus, no_pending, single),
            KeyAction::Quit
        );
        assert_eq!(
            resolve_key(alt_key(KeyCode::Char('1')), focus, no_pending, single),
            KeyAction::FocusChat
        );
        assert_eq!(
            resolve_key(alt_key(KeyCode::Char('2')), focus, no_pending, single),
            KeyAction::FocusSessions
        );
        assert_eq!(
            resolve_key(alt_key(KeyCode::Char('3')), focus, no_pending, single),
            KeyAction::FocusTrace
        );

        // Alt with unknown char → Unhandled
        assert_eq!(
            resolve_key(alt_key(KeyCode::Char('z')), focus, no_pending, single),
            KeyAction::Unhandled
        );
    }

    // -- L3 Ctrl+C interrupts ----------------------------------------------

    #[test]
    fn l3_ctrl_c_interrupts() {
        assert_eq!(
            resolve_key(
                ctrl_key(KeyCode::Char('c')),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::InterruptOrQuit
        );
    }

    #[test]
    fn l3_ctrl_l_redraws() {
        assert_eq!(
            resolve_key(
                ctrl_key(KeyCode::Char('l')),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::Redraw
        );
    }

    // -- L1 Enter sends in single-line -------------------------------------

    #[test]
    fn l1_enter_sends_in_singleline() {
        assert_eq!(
            resolve_key(
                key(KeyCode::Enter),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::SendInput
        );
    }

    // -- L1 Enter creates newline in multi-line ----------------------------

    #[test]
    fn l1_enter_newline_in_multiline() {
        assert_eq!(
            resolve_key(
                key(KeyCode::Enter),
                FocusTarget::Chat,
                false,
                InputMode::MultiLine
            ),
            KeyAction::InputNewline
        );
    }

    // -- Ctrl+Enter sends in multi-line ------------------------------------

    #[test]
    fn ctrl_enter_sends_in_multiline() {
        assert_eq!(
            resolve_key(
                ctrl_key(KeyCode::Enter),
                FocusTarget::Chat,
                false,
                InputMode::MultiLine
            ),
            KeyAction::SendInput
        );
    }

    // -- Permission keys override normal -----------------------------------

    #[test]
    fn permission_keys_override_normal() {
        let pending = true;
        let focus = FocusTarget::Chat;
        let single = InputMode::SingleLine;

        // Y → AllowPermission
        assert_eq!(
            resolve_key(key(KeyCode::Char('y')), focus, pending, single),
            KeyAction::AllowPermission
        );
        // N → DenyPermission
        assert_eq!(
            resolve_key(key(KeyCode::Char('n')), focus, pending, single),
            KeyAction::DenyPermission
        );
        // D → DenyAllPermission
        assert_eq!(
            resolve_key(key(KeyCode::Char('d')), focus, pending, single),
            KeyAction::DenyAllPermission
        );
        // Esc → DenyPermission when pending
        assert_eq!(
            resolve_key(key(KeyCode::Esc), focus, pending, single),
            KeyAction::DenyPermission
        );
        // Uppercase variants
        assert_eq!(
            resolve_key(key(KeyCode::Char('Y')), focus, pending, single),
            KeyAction::AllowPermission
        );
        assert_eq!(
            resolve_key(key(KeyCode::Char('N')), focus, pending, single),
            KeyAction::DenyPermission
        );
        assert_eq!(
            resolve_key(key(KeyCode::Char('D')), focus, pending, single),
            KeyAction::DenyAllPermission
        );
    }

    // -- L4 F5 toggles trace density in Trace focus -------------------------

    #[test]
    fn l4_f5_toggles_trace_in_trace_focus() {
        assert_eq!(
            resolve_key(
                key(KeyCode::F(5)),
                FocusTarget::Trace,
                false,
                InputMode::SingleLine
            ),
            KeyAction::ToggleTraceDensity
        );

        // F5 in Chat focus → Unhandled (L4 is focus-dependent)
        assert_eq!(
            resolve_key(
                key(KeyCode::F(5)),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::Unhandled
        );
    }

    // -- L4 F2 → RenameSession only in Sessions focus ----------------------

    #[test]
    fn l4_f2_rename_in_sessions_focus() {
        assert_eq!(
            resolve_key(
                key(KeyCode::F(2)),
                FocusTarget::Sessions,
                false,
                InputMode::SingleLine
            ),
            KeyAction::RenameSession
        );

        // F2 in Chat → Unhandled
        assert_eq!(
            resolve_key(
                key(KeyCode::F(2)),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::Unhandled
        );
    }

    // -- L4 F1 → Help (global among L4) -----------------------------------

    #[test]
    fn l4_f1_help() {
        assert_eq!(
            resolve_key(
                key(KeyCode::F(1)),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::Help
        );
    }

    // -- TraceDensity cycles -----------------------------------------------

    #[test]
    fn trace_density_cycles() {
        assert_eq!(TraceDensity::Summary.next(), TraceDensity::Expanded);
        assert_eq!(TraceDensity::Expanded.next(), TraceDensity::FullEventStream);
        assert_eq!(
            TraceDensity::FullEventStream.next(),
            TraceDensity::TaskGraph
        );
        assert_eq!(TraceDensity::TaskGraph.next(), TraceDensity::Summary);
    }

    #[test]
    fn trace_density_default_is_summary() {
        assert_eq!(TraceDensity::default(), TraceDensity::Summary);
    }

    // -- Additional coverage -----------------------------------------------

    #[test]
    fn tab_cycles_focus() {
        assert_eq!(
            resolve_key(
                key(KeyCode::Tab),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::FocusCycleNext
        );
    }

    #[test]
    fn esc_escapes() {
        assert_eq!(
            resolve_key(
                key(KeyCode::Esc),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::Escape
        );
    }

    #[test]
    fn enter_in_sessions_selects_session() {
        assert_eq!(
            resolve_key(
                key(KeyCode::Enter),
                FocusTarget::Sessions,
                false,
                InputMode::SingleLine
            ),
            KeyAction::SelectSession
        );
        assert_eq!(
            resolve_key(
                key(KeyCode::Enter),
                FocusTarget::Trace,
                false,
                InputMode::SingleLine
            ),
            KeyAction::FocusCycleNext
        );
    }

    #[test]
    fn arrow_up_down_in_chat_gives_history() {
        assert_eq!(
            resolve_key(
                key(KeyCode::Up),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::InputHistoryUp
        );
        assert_eq!(
            resolve_key(
                key(KeyCode::Down),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::InputHistoryDown
        );
    }

    #[test]
    fn arrow_up_down_in_non_chat_gives_scroll() {
        assert_eq!(
            resolve_key(
                key(KeyCode::Up),
                FocusTarget::Sessions,
                false,
                InputMode::SingleLine
            ),
            KeyAction::ScrollUp
        );
        assert_eq!(
            resolve_key(
                key(KeyCode::Down),
                FocusTarget::Sessions,
                false,
                InputMode::SingleLine
            ),
            KeyAction::ScrollDown
        );
    }

    #[test]
    fn backspace_and_delete() {
        assert_eq!(
            resolve_key(
                key(KeyCode::Backspace),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::InputBackspace
        );
        assert_eq!(
            resolve_key(
                key(KeyCode::Delete),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::InputDelete
        );
    }

    #[test]
    fn x_opens_context_menu() {
        assert_eq!(
            resolve_key(
                key(KeyCode::Char('x')),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::ContextMenu
        );
    }

    #[test]
    fn regular_char_is_input_character() {
        assert_eq!(
            resolve_key(
                key(KeyCode::Char('a')),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::InputCharacter('a')
        );
    }

    #[test]
    fn unknown_key_is_unhandled() {
        assert_eq!(
            resolve_key(
                key(KeyCode::F(12)),
                FocusTarget::Chat,
                false,
                InputMode::SingleLine
            ),
            KeyAction::Unhandled
        );
    }

    #[test]
    fn resolve_paste_returns_input_paste() {
        let action = resolve_paste("hello\nworld".to_string());
        assert_eq!(action, KeyAction::InputPaste("hello\nworld".to_string()));
    }
}
