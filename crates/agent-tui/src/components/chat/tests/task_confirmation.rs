//! Structured task-confirmation prompt behavior.

use super::super::*;
use super::common::*;
use crate::components::TaskConfirmationRequest;
use crate::keybindings::KeyAction;
use agent_core::{TaskConfirmationDecision, TaskConfirmationOption};

fn options() -> Vec<TaskConfirmationOption> {
    vec![
        TaskConfirmationOption {
            id: "small".into(),
            label: "Small fix".into(),
            description: Some("Touch one module".into()),
        },
        TaskConfirmationOption {
            id: "broad".into(),
            label: "Broad pass".into(),
            description: None,
        },
    ]
}

#[test]
fn handle_effect_show_task_confirmation_prompt_enters_wait_state() {
    let mut panel = ChatPanel::new();
    panel.input_content = "draft before prompt".into();
    panel.input_cursor = panel.input_content.len();

    panel.handle_effect(&CrossPanelEffect::ShowTaskConfirmationPrompt(
        TaskConfirmationRequest {
            request_id: "confirm-1".into(),
            prompt: "Choose path".into(),
            options: options(),
            allow_multiple: true,
            allow_custom: true,
        },
    ));

    assert_eq!(panel.input_content, "");
    assert!(matches!(
        panel.input_state,
        InputState::TaskConfirmationWait {
            ref request_id,
            ref prompt,
            ref saved_input,
            ..
        } if request_id == "confirm-1"
            && prompt == "Choose path"
            && saved_input == "draft before prompt"
    ));
}

#[test]
fn task_confirmation_digit_selection_and_enter_submit_decision() {
    let mut panel = ChatPanel::new();
    panel.handle_effect(&CrossPanelEffect::ShowTaskConfirmationPrompt(
        TaskConfirmationRequest {
            request_id: "confirm-2".into(),
            prompt: "Choose path".into(),
            options: options(),
            allow_multiple: false,
            allow_custom: true,
        },
    ));

    let (_effects, _cmds) =
        panel.apply_key_action(KeyAction::InputCharacter('2'), test_ctx_with_session());
    let (_effects, _cmds) =
        panel.apply_key_action(KeyAction::InputCharacter('o'), test_ctx_with_session());
    let (_effects, _cmds) =
        panel.apply_key_action(KeyAction::InputCharacter('k'), test_ctx_with_session());

    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert_eq!(
        effects,
        vec![CrossPanelEffect::DismissTaskConfirmationPrompt]
    );
    assert_eq!(
        cmds,
        vec![Command::DecideTaskConfirmation {
            decision: TaskConfirmationDecision {
                request_id: "confirm-2".into(),
                selected_option_ids: vec!["broad".into()],
                custom_response: Some("ok".into()),
            },
        }]
    );
    assert_eq!(panel.input_state, InputState::Normal);
}
