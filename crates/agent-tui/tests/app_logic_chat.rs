//! TUI App logic integration tests — chat-panel command parsing.
//!
//! Split from the former `app_logic.rs`. Shared helpers live in
//! `app_logic_common`. The two `chat_commands_for_*` helpers are
//! private to this file because they are only used here.

#![allow(unused_imports)]

mod app_logic_common;

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use futures::StreamExt;
use std::sync::Arc;

// ---------------------------------------------------------------------------
// P3 Task 10: `:compact` typed in chat dispatches `Command::CompactSession`
// instead of `Command::SendMessage`.
// ---------------------------------------------------------------------------

#[test]
fn colon_compact_input_dispatches_compact_session_command() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();

    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":compact".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::CompactSession { .. })),
        "expected Command::CompactSession; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
    // Buffer should be cleared after `:compact` is consumed.
    assert!(
        chat.input_content.is_empty(),
        "expected input cleared, got {:?}",
        chat.input_content
    );
}

// ---------------------------------------------------------------------------
// P4 Task 10: `:model <alias>` typed in chat dispatches `Command::SwitchModel`
// instead of `Command::SendMessage`.
// ---------------------------------------------------------------------------

#[test]
fn colon_model_alias_input_dispatches_switch_model_command() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();

    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":model opus".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    let found = commands
        .iter()
        .any(|c| matches!(c, Command::SwitchModel { alias, .. } if alias == "opus"));
    assert!(
        found,
        "expected Command::SwitchModel with alias=opus; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
    // Buffer should be cleared after `:model <alias>` is consumed.
    assert!(
        chat.input_content.is_empty(),
        "expected input cleared, got {:?}",
        chat.input_content
    );
}

#[test]
fn colon_model_without_alias_falls_through_as_chat_message() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":model".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    // `:model` without an alias falls through to SendMessage (user gets
    // feedback the command was malformed — no silent swallow).
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected SendMessage fallback; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SwitchModel { .. })),
        "expected NO SwitchModel without alias; got {commands:?}"
    );
}

fn chat_commands_for_input(input: &str) -> Vec<agent_tui::components::Command> {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for character in input.chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(character), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);
    commands
}

fn chat_commands_for_project_input(
    input: &str,
) -> (agent_core::ProjectId, Vec<agent_tui::components::Command>) {
    use agent_core::projection::SessionProjection;
    use agent_core::{ProjectId, ProjectSessionVisibility, SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{EventContext, FocusTarget, SessionInfo, SessionState};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let project_id = ProjectId::new();
    let projection = SessionProjection::default();
    let sessions = vec![SessionInfo {
        id: session_id.clone(),
        title: "project session".into(),
        model_profile: "fake".into(),
        state: SessionState::Idle,
        pinned: false,
        archived: false,
        project_id: Some(project_id.clone()),
        worktree_path: Some("/tmp/project".into()),
        branch: Some("main".into()),
        visibility: Some(ProjectSessionVisibility::Visible),
    }];
    let current_session_id = Some(session_id);
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &sessions,
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &current_session_id,
    };

    let mut chat = ChatPanel::new();
    for character in input.chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(character), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);
    (project_id, commands)
}

#[test]
fn colon_attach_then_send_carries_attachment_payload() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let canonical = manifest.canonicalize().unwrap();
    let mut chat = ChatPanel::new();
    for character in format!(":attach {}", manifest.display()).chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(character), &ctx);
    }
    let (_effects, attach_commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);
    assert!(
        attach_commands.is_empty(),
        "attach should only update composer state, got {attach_commands:?}"
    );

    for character in "summarize this".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(character), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        Command::SendMessage {
            content,
            attachments,
            ..
        } if content == "summarize this"
            && attachments.len() == 1
            && attachments[0].path == canonical.display().to_string()
            && attachments[0].name == "Cargo.toml"
            && attachments[0].mime_type == "application/toml"
    ));
}

#[test]
fn colon_skills_input_dispatches_list_skills_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skills");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::ListSkills)),
        "expected Command::ListSkills; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_plugins_input_dispatches_open_plugins_overlay_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":plugins");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::OpenPluginsOverlay)),
        "expected Command::OpenPluginsOverlay; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_agents_input_dispatches_open_agent_settings_overlay_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":agents");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::OpenAgentSettingsOverlay)),
        "expected Command::OpenAgentSettingsOverlay; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_instructions_input_dispatches_open_instructions_overlay_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":instructions");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::OpenInstructionsOverlay)),
        "expected Command::OpenInstructionsOverlay; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_hooks_input_dispatches_open_hooks_overlay_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":hooks");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::OpenHooksOverlay)),
        "expected Command::OpenHooksOverlay; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_project_draft_input_dispatches_create_project_draft_command() {
    use agent_tui::components::Command;

    let (expected_project_id, commands) = chat_commands_for_project_input(":project draft");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::CreateProjectDraftSession { project_id } if project_id == &expected_project_id)
        ),
        "expected CreateProjectDraftSession; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_project_create_input_dispatches_create_blank_project_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":project create Alpha Workbench");

    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::CreateBlankProject { display_name: Some(display_name) }
                if display_name == "Alpha Workbench"
        )),
        "expected CreateBlankProject; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_project_import_input_dispatches_add_existing_project_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":project import /tmp/kairox-existing");

    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::AddExistingProject { path } if path == "/tmp/kairox-existing"
        )),
        "expected AddExistingProject; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_project_worktree_input_dispatches_create_worktree_command() {
    use agent_tui::components::Command;

    let (expected_project_id, commands) =
        chat_commands_for_project_input(":project worktree feat/tui");

    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::CreateProjectWorktreeSession { project_id, branch_name }
                if project_id == &expected_project_id && branch_name == "feat/tui"
        )),
        "expected CreateProjectWorktreeSession; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_skill_show_input_dispatches_show_skill_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill show test-driven-rust");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::ShowSkill { skill_id } if skill_id == "test-driven-rust")
        ),
        "expected Command::ShowSkill for test-driven-rust; got {commands:?}"
    );
}

#[test]
fn colon_skill_activate_input_dispatches_activate_skill_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill activate test-driven-rust");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::ActivateSkill { skill_id, .. } if skill_id == "test-driven-rust")
        ),
        "expected Command::ActivateSkill for test-driven-rust; got {commands:?}"
    );
}

#[test]
fn colon_skill_deactivate_input_dispatches_deactivate_skill_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill deactivate test-driven-rust");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::DeactivateSkill { skill_id, .. } if skill_id == "test-driven-rust")
        ),
        "expected Command::DeactivateSkill for test-driven-rust; got {commands:?}"
    );
}

#[test]
fn colon_skill_catalog_input_dispatches_list_skill_catalog_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill catalog review");

    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::ListSkillCatalog {
                keyword: Some(keyword),
                sources: None
            } if keyword == "review"
        )),
        "expected Command::ListSkillCatalog for review; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );

    let commands = chat_commands_for_input(":skill catalog");
    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::ListSkillCatalog {
                keyword: None,
                sources: None
            }
        )),
        "expected Command::ListSkillCatalog without keyword; got {commands:?}"
    );
}

#[test]
fn colon_skill_install_github_input_dispatches_github_install_command() {
    use agent_core::facade::SkillInstallTarget;
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill install github owner/review");

    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::InstallGithubSkill { request }
                if request.source == "owner/review" && request.target == SkillInstallTarget::User
        )),
        "expected Command::InstallGithubSkill for owner/review; got {commands:?}"
    );
}

#[test]
fn skill_catalog_install_update_delete_command_variants_carry_payloads() {
    use agent_core::facade::{
        InstallGithubSkillRequest, InstallRemoteSkillRequest, SkillInstallTarget, SkillUpdateState,
    };
    use agent_tui::components::Command;

    let install = Command::InstallRemoteSkill {
        request: InstallRemoteSkillRequest {
            package: "review".to_string(),
            source: "skillhub".to_string(),
            target: SkillInstallTarget::Project,
            package_url: Some("https://example.test/review.zip".to_string()),
        },
    };
    let github_install = Command::InstallGithubSkill {
        request: InstallGithubSkillRequest {
            source: "owner/review".to_string(),
            target: SkillInstallTarget::User,
        },
    };
    let update = Command::UpdateSkillSettings {
        skill_id: "review".to_string(),
    };
    let delete = Command::DeleteSkillSettings {
        skill_id: "review".to_string(),
    };

    assert!(matches!(
        install,
        Command::InstallRemoteSkill { request }
            if request.package == "review"
                && request.source == "skillhub"
                && request.target == SkillInstallTarget::Project
                && request.package_url.as_deref() == Some("https://example.test/review.zip")
    ));
    assert!(matches!(
        github_install,
        Command::InstallGithubSkill { request }
            if request.source == "owner/review" && request.target == SkillInstallTarget::User
    ));
    assert!(matches!(
        update,
        Command::UpdateSkillSettings { skill_id } if skill_id == "review"
    ));
    assert!(matches!(
        delete,
        Command::DeleteSkillSettings { skill_id } if skill_id == "review"
    ));

    let update_state = SkillUpdateState::UpdateAvailable;
    assert_eq!(update_state, SkillUpdateState::UpdateAvailable);
}

#[test]
fn colon_monitors_input_dispatches_monitor_list_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":monitors");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::MonitorList)),
        "expected Command::MonitorList; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_monitor_stop_input_dispatches_monitor_stop_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":monitor stop mon_1");

    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::MonitorStop { monitor_id } if monitor_id == "mon_1"
        )),
        "expected Command::MonitorStop with mon_1; got {commands:?}"
    );
}

#[test]
fn colon_monitor_stop_without_id_sends_as_message() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":monitor stop ");

    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::MonitorStop { .. })),
        "expected NO MonitorStop for empty id; got {commands:?}"
    );
}
