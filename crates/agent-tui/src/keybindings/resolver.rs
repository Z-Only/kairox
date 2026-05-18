use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app_state::InputMode;
use crate::components::FocusTarget;

use super::KeyAction;

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

    if mods.contains(KeyModifiers::CONTROL) {
        return match code {
            KeyCode::Char('c') => KeyAction::InterruptOrQuit,
            KeyCode::Char('l') => KeyAction::Redraw,
            KeyCode::Enter => KeyAction::SendInput,
            _ => KeyAction::Unhandled,
        };
    }

    match code {
        KeyCode::F(1) => return KeyAction::Help,
        KeyCode::F(2) if focus == FocusTarget::Sessions => return KeyAction::RenameSession,
        KeyCode::F(5) if focus == FocusTarget::Trace => return KeyAction::ToggleTraceDensity,
        _ => {}
    }

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
        KeyCode::Enter => match focus {
            FocusTarget::Chat => match input_mode {
                InputMode::SingleLine => KeyAction::SendInput,
                InputMode::MultiLine => KeyAction::InputNewline,
            },
            FocusTarget::Sessions => KeyAction::SelectSession,
            _ => KeyAction::FocusCycleNext,
        },
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
pub fn resolve_paste(text: String) -> KeyAction {
    KeyAction::InputPaste(text)
}
