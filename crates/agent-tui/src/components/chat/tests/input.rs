//! Composer input behavior: typed characters, history navigation, send
//! on Enter, multiline mode toggling, escape handling, and focus state.

use super::super::*;
use super::common::*;
use crate::keybindings::KeyAction;

#[test]
fn input_character_appends_to_content() {
    let mut panel = ChatPanel::new();
    let (effects, cmds) = panel.apply_key_action(KeyAction::InputCharacter('a'), test_ctx());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_content, "a");
    assert_eq!(panel.input_cursor, 1);

    panel.apply_key_action(KeyAction::InputCharacter('b'), test_ctx());
    assert_eq!(panel.input_content, "ab");
    assert_eq!(panel.input_cursor, 2);
}

#[test]
fn backspace_removes_character() {
    let mut panel = ChatPanel::new();
    panel.apply_key_action(KeyAction::InputCharacter('x'), test_ctx());
    panel.apply_key_action(KeyAction::InputCharacter('y'), test_ctx());
    assert_eq!(panel.input_content, "xy");
    assert_eq!(panel.input_cursor, 2);

    let (effects, cmds) = panel.apply_key_action(KeyAction::InputBackspace, test_ctx());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_content, "x");
    assert_eq!(panel.input_cursor, 1);

    // Backspace at start does nothing
    panel.apply_key_action(KeyAction::InputBackspace, test_ctx());
    assert_eq!(panel.input_content, "");
    assert_eq!(panel.input_cursor, 0);
    panel.apply_key_action(KeyAction::InputBackspace, test_ctx());
    assert_eq!(panel.input_content, "");
    assert_eq!(panel.input_cursor, 0);
}

#[test]
fn toggle_input_mode_switches() {
    let mut panel = ChatPanel::new();
    assert_eq!(panel.input_mode, InputMode::SingleLine);

    let (effects, cmds) = panel.apply_key_action(KeyAction::ToggleInputMode, test_ctx());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_mode, InputMode::MultiLine);

    panel.apply_key_action(KeyAction::ToggleInputMode, test_ctx());
    assert_eq!(panel.input_mode, InputMode::SingleLine);
}

#[test]
fn history_navigation_works() {
    let mut panel = ChatPanel::new();

    // Pre-populate history
    panel.input_history = vec![
        "first message".to_string(),
        "second message".to_string(),
        "third message".to_string(),
    ];

    // HistoryUp from live position -> goes to most recent (index 2)
    let (effects, cmds) = panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_history_index, Some(2));
    assert_eq!(panel.input_content, "third message");
    assert_eq!(panel.input_cursor, "third message".len());

    // HistoryUp again -> index 1
    panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
    assert_eq!(panel.input_history_index, Some(1));
    assert_eq!(panel.input_content, "second message");

    // HistoryUp again -> index 0 (oldest)
    panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
    assert_eq!(panel.input_history_index, Some(0));
    assert_eq!(panel.input_content, "first message");

    // HistoryUp at oldest -> stays at index 0
    panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
    assert_eq!(panel.input_history_index, Some(0));
    assert_eq!(panel.input_content, "first message");

    // HistoryDown -> index 1
    panel.apply_key_action(KeyAction::InputHistoryDown, test_ctx());
    assert_eq!(panel.input_history_index, Some(1));
    assert_eq!(panel.input_content, "second message");

    // HistoryDown to index 2
    panel.apply_key_action(KeyAction::InputHistoryDown, test_ctx());
    assert_eq!(panel.input_history_index, Some(2));
    assert_eq!(panel.input_content, "third message");

    // HistoryDown from index 2 -> back to live position
    panel.apply_key_action(KeyAction::InputHistoryDown, test_ctx());
    assert_eq!(panel.input_history_index, None);
    assert_eq!(panel.input_content, "");
}

#[test]
fn send_input_clears_content_and_emits_command() {
    let mut panel = ChatPanel::new();
    // Type some content
    panel.apply_key_action(KeyAction::InputCharacter('h'), test_ctx());
    panel.apply_key_action(KeyAction::InputCharacter('i'), test_ctx());
    assert_eq!(panel.input_content, "hi");

    // Send
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert!(effects.is_empty());
    assert_eq!(cmds.len(), 1);

    match &cmds[0] {
        Command::SendMessage {
            content,
            attachments,
            ..
        } => {
            assert_eq!(content, "hi");
            assert!(attachments.is_empty());
        }
        other => panic!("expected SendMessage, got {:?}", other),
    }

    // Content should be cleared
    assert_eq!(panel.input_content, "");
    assert_eq!(panel.input_cursor, 0);

    // History should contain the sent message
    assert_eq!(panel.input_history, vec!["hi"]);

    // History index should be reset
    assert_eq!(panel.input_history_index, None);
}

#[test]
fn send_input_empty_does_nothing() {
    let mut panel = ChatPanel::new();
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
}

#[test]
fn send_input_no_sessions_no_command() {
    let mut panel = ChatPanel::new();
    panel.apply_key_action(KeyAction::InputCharacter('x'), test_ctx());
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx());
    // No sessions -> no command emitted, but content is still consumed
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    // Content is still pushed to history and cleared
    assert_eq!(panel.input_content, "");
    assert_eq!(panel.input_history, vec!["x"]);
}

#[test]
fn escape_multiline_empty_switches_to_singleline() {
    let mut panel = ChatPanel::new();
    panel.input_mode = InputMode::MultiLine;
    assert!(panel.input_content.is_empty());

    let (effects, cmds) = panel.apply_key_action(KeyAction::Escape, test_ctx());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_mode, InputMode::SingleLine);
}

#[test]
fn escape_multiline_nonempty_does_not_switch() {
    let mut panel = ChatPanel::new();
    panel.input_mode = InputMode::MultiLine;
    panel.apply_key_action(KeyAction::InputCharacter('a'), test_ctx());

    panel.apply_key_action(KeyAction::Escape, test_ctx());
    assert_eq!(panel.input_mode, InputMode::MultiLine);
}

#[test]
fn newline_only_in_multiline() {
    let mut panel = ChatPanel::new();
    // SingleLine: newline is a no-op
    panel.apply_key_action(KeyAction::InputNewline, test_ctx());
    assert_eq!(panel.input_content, "");

    // Switch to MultiLine
    panel.apply_key_action(KeyAction::ToggleInputMode, test_ctx());
    panel.apply_key_action(KeyAction::InputNewline, test_ctx());
    assert_eq!(panel.input_content, "\n");
    assert_eq!(panel.input_cursor, 1);
}

#[test]
fn send_input_idle_session_emits_send_not_queue() {
    let mut panel = ChatPanel::new();
    panel.apply_key_action(KeyAction::InputCharacter('h'), test_ctx_with_session());
    let (_, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert!(cmds
        .iter()
        .any(|c| matches!(c, Command::SendMessage { .. })));
    assert!(panel.message_queue.is_empty());
}

#[test]
fn focused_and_set_focused() {
    let mut panel = ChatPanel::new();
    assert!(!panel.focused());
    panel.set_focused(true);
    assert!(panel.focused());
    panel.set_focused(false);
    assert!(!panel.focused());
}
