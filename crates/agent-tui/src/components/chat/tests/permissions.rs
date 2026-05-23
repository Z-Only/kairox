//! Permission prompt behavior: keyboard allow/deny resolution and the
//! `handle_effect` paths that show or dismiss the modal state.

use super::super::*;
use super::common::*;
use crate::components::PermissionRequest;
use crate::keybindings::KeyAction;

#[test]
fn permission_wait_state_allows_deny() {
    let mut panel = ChatPanel::new();
    panel.input_state = InputState::PermissionWait {
        request_id: "req-1".to_string(),
        pending_prompt: "rm file?".to_string(),
    };

    let (effects, cmds) = panel.apply_key_action(KeyAction::DenyPermission, test_ctx());
    assert_eq!(effects, vec![CrossPanelEffect::DismissPermissionPrompt]);
    assert_eq!(
        cmds,
        vec![Command::DecidePermission {
            request_id: "req-1".to_string(),
            approved: false,
        }]
    );
    assert_eq!(panel.input_state, InputState::Normal);

    // Allow also works
    panel.input_state = InputState::PermissionWait {
        request_id: "req-2".to_string(),
        pending_prompt: "write file?".to_string(),
    };
    let (effects2, cmds2) = panel.apply_key_action(KeyAction::AllowPermission, test_ctx());
    assert_eq!(effects2, vec![CrossPanelEffect::DismissPermissionPrompt]);
    assert_eq!(
        cmds2,
        vec![Command::DecidePermission {
            request_id: "req-2".to_string(),
            approved: true,
        }]
    );
    assert_eq!(panel.input_state, InputState::Normal);

    // DenyAllPermission also resolves to deny
    panel.input_state = InputState::PermissionWait {
        request_id: "req-3".to_string(),
        pending_prompt: "run cmd?".to_string(),
    };
    let (effects3, cmds3) = panel.apply_key_action(KeyAction::DenyAllPermission, test_ctx());
    assert_eq!(effects3, vec![CrossPanelEffect::DismissPermissionPrompt]);
    assert_eq!(
        cmds3,
        vec![Command::DecidePermission {
            request_id: "req-3".to_string(),
            approved: false,
        }]
    );
    assert_eq!(panel.input_state, InputState::Normal);
}

#[test]
fn handle_effect_show_permission_prompt_write_level() {
    let mut panel = ChatPanel::new();
    let req = PermissionRequest {
        request_id: "r1".to_string(),
        tool_id: "write_file".to_string(),
        tool_preview: "write to foo.txt".to_string(),
        risk_level: RiskLevel::Write,
    };
    panel.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(req));
    assert!(matches!(
        panel.input_state,
        InputState::PermissionWait { ref request_id, .. } if request_id == "r1"
    ));
}

#[test]
fn handle_effect_show_permission_prompt_destructive_ignored() {
    let mut panel = ChatPanel::new();
    let req = PermissionRequest {
        request_id: "r2".to_string(),
        tool_id: "delete_file".to_string(),
        tool_preview: "rm -rf /".to_string(),
        risk_level: RiskLevel::Destructive,
    };
    panel.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(req));
    assert_eq!(panel.input_state, InputState::Normal);
}

#[test]
fn handle_effect_dismiss_permission_prompt() {
    let mut panel = ChatPanel::new();
    panel.input_state = InputState::PermissionWait {
        request_id: "r1".to_string(),
        pending_prompt: "test".to_string(),
    };
    panel.handle_effect(&CrossPanelEffect::DismissPermissionPrompt);
    assert_eq!(panel.input_state, InputState::Normal);
}
