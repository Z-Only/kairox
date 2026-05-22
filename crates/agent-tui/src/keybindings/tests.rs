use super::*;
use crate::app_state::InputMode;
use crate::components::FocusTarget;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
        resolve_key(alt_key(KeyCode::Char('c')), focus, no_pending, single),
        KeyAction::ToggleContextDetails
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
        resolve_key(alt_key(KeyCode::Char('h')), focus, no_pending, single),
        KeyAction::ToggleHooksOverlay
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
fn l3_ctrl_l_toggles_model_overlay() {
    assert_eq!(
        resolve_key(
            ctrl_key(KeyCode::Char('l')),
            FocusTarget::Chat,
            false,
            InputMode::SingleLine
        ),
        KeyAction::ToggleModelOverlay
    );
}

#[test]
fn l3_ctrl_g_toggles_plugin_overlay() {
    assert_eq!(
        resolve_key(
            ctrl_key(KeyCode::Char('g')),
            FocusTarget::Chat,
            false,
            InputMode::SingleLine
        ),
        KeyAction::TogglePluginsOverlay
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

#[test]
fn sessions_focus_a_opens_archive_manager() {
    assert_eq!(
        resolve_key(
            key(KeyCode::Char('a')),
            FocusTarget::Sessions,
            false,
            InputMode::SingleLine
        ),
        KeyAction::OpenArchiveManager
    );
    assert_eq!(
        resolve_key(
            key(KeyCode::Char('A')),
            FocusTarget::Sessions,
            false,
            InputMode::SingleLine
        ),
        KeyAction::OpenArchiveManager
    );
}

#[test]
fn trace_focus_memory_browser_keys_resolve() {
    assert_eq!(
        resolve_key(
            key(KeyCode::Char('/')),
            FocusTarget::Trace,
            false,
            InputMode::SingleLine
        ),
        KeyAction::StartMemorySearch
    );
    assert_eq!(
        resolve_key(
            key(KeyCode::Char('s')),
            FocusTarget::Trace,
            false,
            InputMode::SingleLine
        ),
        KeyAction::CycleMemoryScope
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
