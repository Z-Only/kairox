//! Attachment plumbing: `:attach`/`:detach` slash commands, attachment
//! payloads on send, compact label formatting, and `@`-prefixed file
//! mention completion against the workspace listing.

use super::super::*;
use super::common::*;
use crate::keybindings::KeyAction;

#[test]
fn attach_command_adds_pending_attachment() {
    let manifest = agent_tui_manifest_path();
    let canonical = manifest.canonicalize().unwrap();
    let mut panel = ChatPanel::new();

    for ch in format!(":attach {}", manifest.display()).chars() {
        panel.apply_key_action(KeyAction::InputCharacter(ch), test_ctx_with_session());
    }
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.pending_attachments.len(), 1);
    assert_eq!(
        panel.pending_attachments[0],
        agent_core::AttachmentInfo {
            path: canonical.display().to_string(),
            name: "Cargo.toml".to_string(),
            mime_type: "application/toml".to_string(),
        }
    );
    assert!(panel.input_content.is_empty());
}

#[test]
fn detach_command_clears_pending_attachments() {
    let mut panel = ChatPanel::new();
    panel
        .pending_attachments
        .push(fixture_attachment("first.txt"));
    panel
        .pending_attachments
        .push(fixture_attachment("second.txt"));

    for ch in ":detach".chars() {
        panel.apply_key_action(KeyAction::InputCharacter(ch), test_ctx_with_session());
    }
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert!(panel.pending_attachments.is_empty());
    assert!(panel.input_content.is_empty());
}

#[test]
fn send_input_emits_attachment_payloads_and_clears_pending() {
    let mut panel = ChatPanel::new();
    panel
        .pending_attachments
        .push(fixture_attachment("notes.txt"));
    for ch in "review this".chars() {
        panel.apply_key_action(KeyAction::InputCharacter(ch), test_ctx_with_session());
    }

    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert!(effects.is_empty());
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Command::SendMessage {
            content,
            attachments,
            ..
        } => {
            assert_eq!(content, "review this");
            assert_eq!(attachments, &vec![fixture_attachment("notes.txt")]);
        }
        other => panic!("expected SendMessage, got {other:?}"),
    }
    assert!(panel.pending_attachments.is_empty());
}

#[test]
fn attachment_only_send_emits_message() {
    let mut panel = ChatPanel::new();
    panel
        .pending_attachments
        .push(fixture_attachment("screenshot.png"));

    let (_effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert_eq!(cmds.len(), 1);
    assert!(matches!(
        &cmds[0],
        Command::SendMessage {
            content,
            attachments,
            ..
        } if content.is_empty() && attachments == &vec![fixture_attachment("screenshot.png")]
    ));
    assert!(panel.pending_attachments.is_empty());
}

#[test]
fn attachment_labels_are_compact() {
    assert_eq!(
        format_attachment_labels(&[
            fixture_attachment("one.txt"),
            fixture_attachment("two.txt"),
            fixture_attachment("three.txt"),
        ]),
        "[one.txt] [two.txt] [+1]"
    );
}

#[test]
fn file_mentions_filter_workspace_files_and_attach_selection() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let selected = root.join("src/main.rs").canonicalize().unwrap();
    let mut panel = ChatPanel::new();
    panel.set_workspace_files(
        root,
        vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
    );

    for ch in "@main".chars() {
        panel.apply_key_action(KeyAction::InputCharacter(ch), test_ctx_with_session());
    }

    assert!(panel.file_mentions_visible());
    assert_eq!(panel.file_mention_matches(), &["src/main.rs".to_string()]);

    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_content, "@src/main.rs ");
    assert_eq!(panel.input_cursor, panel.input_content.len());
    assert!(!panel.file_mentions_visible());
    assert_eq!(
        panel.pending_attachments,
        vec![agent_core::AttachmentInfo {
            path: selected.display().to_string(),
            name: "main.rs".to_string(),
            mime_type: "text/x-rust".to_string(),
        }]
    );
}

#[test]
fn file_mentions_can_select_next_match_before_accepting() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let selected = root.join("src/main.rs").canonicalize().unwrap();
    let mut panel = ChatPanel::new();
    panel.set_workspace_files(
        root,
        vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
    );

    panel.apply_key_action(KeyAction::InputCharacter('@'), test_ctx_with_session());
    assert_eq!(
        panel.file_mention_matches(),
        &["Cargo.toml".to_string(), "src/main.rs".to_string()]
    );

    panel.apply_key_action(KeyAction::InputHistoryDown, test_ctx_with_session());
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_content, "@src/main.rs ");
    assert_eq!(
        panel.pending_attachments,
        vec![agent_core::AttachmentInfo {
            path: selected.display().to_string(),
            name: "main.rs".to_string(),
            mime_type: "text/x-rust".to_string(),
        }]
    );
}
