//! Local message-queue mechanics: enqueue when the session is busy,
//! FIFO drain, selection/reorder/delete/restore controls, the "send
//! now" command, and the `:queue` slash-command shortcuts.

use super::super::*;
use super::common::*;
use crate::components::QueuedMessage;
use crate::keybindings::KeyAction;

#[test]
fn queues_message_while_session_running() {
    let mut panel = ChatPanel::new();
    panel
        .pending_attachments
        .push(fixture_attachment("queued.txt"));
    panel.apply_key_action(KeyAction::InputCharacter('h'), test_ctx_busy_session());
    panel.apply_key_action(KeyAction::InputCharacter('i'), test_ctx_busy_session());
    assert_eq!(panel.input_content, "hi");

    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_busy_session());

    assert!(effects.is_empty());
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "must not emit SendMessage while session busy, got {:?}",
        cmds
    );

    assert_eq!(panel.message_queue.len(), 1);
    assert_eq!(panel.message_queue[0].content, "hi");
    assert_eq!(
        panel.message_queue[0].attachments,
        vec![fixture_attachment("queued.txt")]
    );

    // input cleared so user can keep typing
    assert_eq!(panel.input_content, "");
    assert_eq!(panel.input_cursor, 0);
    assert!(panel.pending_attachments.is_empty());
}

#[test]
fn queue_drain_returns_all_pending_in_fifo_order() {
    let mut panel = ChatPanel::new();
    panel.message_queue.push(QueuedMessage {
        content: "first".to_string(),
        attachments: vec![fixture_attachment("first.txt")],
    });
    panel.message_queue.push(QueuedMessage {
        content: "second".to_string(),
        attachments: vec![fixture_attachment("second.txt")],
    });

    let drained = panel.drain_queue();
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].content, "first");
    assert_eq!(
        drained[0].attachments,
        vec![fixture_attachment("first.txt")]
    );
    assert_eq!(drained[1].content, "second");
    assert_eq!(
        drained[1].attachments,
        vec![fixture_attachment("second.txt")]
    );
    assert!(panel.message_queue.is_empty());
}

#[test]
fn queue_selection_and_reorder_controls_target_visible_messages() {
    let mut panel = ChatPanel::new();
    for content in ["first", "second", "third"] {
        panel.message_queue.push(QueuedMessage {
            content: content.to_string(),
            attachments: Vec::new(),
        });
    }

    assert_eq!(panel.selected_queue_index(), Some(0));
    assert!(panel.select_next_queued_message());
    assert_eq!(panel.selected_queue_index(), Some(1));

    assert!(panel.move_selected_queued_message_down());
    assert_eq!(
        panel
            .message_queue
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "third", "second"]
    );
    assert_eq!(panel.selected_queue_index(), Some(2));

    assert!(panel.move_selected_queued_message_up());
    assert_eq!(
        panel
            .message_queue
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second", "third"]
    );
    assert_eq!(panel.selected_queue_index(), Some(1));

    assert!(panel.select_previous_queued_message());
    assert_eq!(panel.selected_queue_index(), Some(0));
}

#[test]
fn queue_delete_keeps_current_draft_and_clamps_selection() {
    let mut panel = ChatPanel::new();
    panel.input_content = "current draft".to_string();
    panel.input_cursor = panel.input_content.len();
    for content in ["first", "second"] {
        panel.message_queue.push(QueuedMessage {
            content: content.to_string(),
            attachments: Vec::new(),
        });
    }
    panel.select_next_queued_message();

    let removed = panel.delete_selected_queued_message();

    assert_eq!(
        removed.map(|message| message.content),
        Some("second".to_string())
    );
    assert_eq!(panel.input_content, "current draft");
    assert_eq!(panel.selected_queue_index(), Some(0));
    assert_eq!(panel.message_queue[0].content, "first");
}

#[test]
fn queue_restore_selected_message_moves_it_into_composer() {
    let mut panel = ChatPanel::new();
    panel.input_content = "draft to replace".to_string();
    panel
        .pending_attachments
        .push(fixture_attachment("draft.txt"));
    panel.message_queue.push(QueuedMessage {
        content: "first".to_string(),
        attachments: Vec::new(),
    });
    panel.message_queue.push(QueuedMessage {
        content: "second".to_string(),
        attachments: vec![fixture_attachment("second.txt")],
    });
    panel.select_next_queued_message();

    let restored = panel.restore_selected_queued_message_for_edit();

    assert!(restored);
    assert_eq!(panel.input_content, "second");
    assert_eq!(panel.input_cursor, "second".len());
    assert_eq!(
        panel.pending_attachments,
        vec![fixture_attachment("second.txt")]
    );
    assert_eq!(
        panel
            .message_queue
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>(),
        vec!["first"]
    );
}

#[test]
fn queue_send_now_command_keeps_message_until_runtime_accepts_it() {
    let mut panel = ChatPanel::new();
    panel.message_queue.push(QueuedMessage {
        content: "first".to_string(),
        attachments: Vec::new(),
    });
    panel.message_queue.push(QueuedMessage {
        content: "second".to_string(),
        attachments: vec![fixture_attachment("second.txt")],
    });
    panel.select_next_queued_message();

    let (_, cmds) = panel.apply_key_action(
        KeyAction::ApplyQueueAction(crate::components::QueueAction::SendSelectedNow),
        test_ctx_with_session(),
    );

    assert_eq!(panel.message_queue.len(), 2);
    assert!(matches!(
        cmds.as_slice(),
        [Command::SendQueuedMessageNow { queue_index: 1, .. }]
    ));
}

#[test]
fn local_queue_slash_commands_edit_delete_and_reorder() {
    let mut panel = ChatPanel::new();
    for content in ["first", "second", "third"] {
        panel.message_queue.push(QueuedMessage {
            content: content.to_string(),
            attachments: Vec::new(),
        });
    }

    panel.input_content = ":queue next".to_string();
    panel.input_cursor = panel.input_content.len();
    panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert_eq!(panel.selected_queue_index(), Some(1));

    panel.input_content = ":queue up".to_string();
    panel.input_cursor = panel.input_content.len();
    panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert_eq!(
        panel
            .message_queue
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>(),
        vec!["second", "first", "third"]
    );

    panel.input_content = ":queue delete".to_string();
    panel.input_cursor = panel.input_content.len();
    panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert_eq!(
        panel
            .message_queue
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "third"]
    );
}
