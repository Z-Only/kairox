//! TUI App logic integration tests — model profile settings, command-status
//! transcript hygiene, project manager, and settings utilities.
//!
//! Split from the former `app_logic.rs`. Shared helpers live in
//! `app_logic_common`.

#![allow(unused_imports)]

mod app_logic_common;

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use app_logic_common::{test_project, unique_temp_dir, TuiMcpFakeFacade};
use futures::StreamExt;
use std::sync::Arc;

#[tokio::test]
async fn tui_model_profile_settings_commands_call_facade_and_report_results() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::SaveProfileSettings {
                input: agent_core::facade::ProfileSettingsInput {
                    alias: "local".into(),
                    provider: "fake".into(),
                    model_id: "local-model".into(),
                    enabled: true,
                    context_window: Some(128000),
                    output_limit: Some(8192),
                    temperature: Some(0.2),
                    top_p: Some(0.9),
                    top_k: Some(40),
                    max_tokens: Some(4096),
                    base_url: Some("http://localhost:11434/v1".into()),
                    api_key_env: Some("LOCAL_LLM_API_KEY".into()),
                    client_identity: None,
                },
            },
            Command::SetProfileEnabled {
                alias: "fast".into(),
                enabled: false,
            },
            Command::MoveProfileInOrder {
                alias: "fast".into(),
                direction: 1,
            },
            Command::TestModelProfile {
                alias: "fast".into(),
            },
            Command::DeleteProfileSettings {
                alias: "fast".into(),
            },
        ],
    )
    .await;

    let calls = runtime.calls();
    for expected in [
        "upsert_profile_settings:local:fake:local-model",
        "set_profile_enabled:fast:false",
        "move_profile_in_order:fast:1",
        "list_profile_settings:Some(\"user\")",
        "delete_profile_settings:fast",
    ] {
        assert!(
            calls.iter().any(|call| call == expected),
            "expected call {expected}, got {calls:?}"
        );
    }

    assert!(
        app.state
            .status_log
            .iter()
            .map(|entry| entry.message.as_str())
            .any(|message| message.contains("model profile fast connectivity ok")),
        "expected model test result in status log; got {:?}",
        app.state.status_log
    );
}

#[tokio::test]
async fn app_logic_command_status_success_does_not_pollute_chat_transcript() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());
    let initial_chat_count = app.state.current_session.messages.len();

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::SetProfileEnabled {
            alias: "fast".into(),
            enabled: false,
        }],
    )
    .await;

    assert_eq!(app.state.current_session.messages.len(), initial_chat_count);
    assert_eq!(
        app.state
            .latest_status_message()
            .map(|entry| entry.message.as_str()),
        Some("disabled model profile fast")
    );
}

#[tokio::test]
async fn app_logic_command_status_failure_does_not_pollute_chat_transcript() {
    use agent_core::{ConfigScope, WorkspaceId};
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());
    let initial_chat_count = app.state.current_session.messages.len();

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::DeleteHookSettings {
            scope: ConfigScope::Builtin,
            event: "PreToolUse".into(),
            id: "readonly".into(),
        }],
    )
    .await;

    assert_eq!(app.state.current_session.messages.len(), initial_chat_count);
    let latest = app
        .state
        .latest_status_message()
        .map(|entry| entry.message.as_str())
        .unwrap_or_default();
    assert!(
        latest.contains("[hooks delete error:"),
        "expected hook delete error in status log, got {latest:?}"
    );
}

#[tokio::test]
async fn tui_project_manager_commands_call_facade_and_update_state() {
    use agent_core::{ProjectId, WorkspaceId};
    use agent_tui::app::App;
    use agent_tui::components::{Command, ProjectInfo};

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let workspace_id = WorkspaceId::new();
    let mut app = App::new("fake", workspace_id.clone());
    let alpha = ProjectId::from_string("prj_alpha".to_string());
    let beta = ProjectId::from_string("prj_beta".to_string());
    app.state.projects = vec![
        ProjectInfo {
            id: alpha.clone(),
            display_name: "alpha".into(),
            root_path: "/tmp/alpha".into(),
            expanded: true,
            git_status: None,
            instruction_summary: None,
        },
        ProjectInfo {
            id: beta.clone(),
            display_name: "beta".into(),
            root_path: "/tmp/beta".into(),
            expanded: true,
            git_status: None,
            instruction_summary: None,
        },
    ];

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::CreateBlankProject {
                display_name: Some("Gamma".into()),
            },
            Command::AddExistingProject {
                path: "/tmp/imported".into(),
            },
            Command::RenameProject {
                project_id: alpha.clone(),
                display_name: "Alpha renamed".into(),
            },
            Command::MoveProject {
                project_id: beta.clone(),
                direction: -1,
            },
            Command::SetProjectExpanded {
                project_id: alpha.clone(),
                expanded: false,
            },
            Command::RefreshProjectGitStatus {
                project_id: alpha.clone(),
            },
            Command::InitProjectGit {
                project_id: alpha.clone(),
            },
            Command::ShowProjectInstructions {
                project_id: alpha.clone(),
            },
            Command::RemoveProject {
                project_id: alpha.clone(),
            },
        ],
    )
    .await;

    let calls = runtime.calls();
    for expected in [
        format!("create_blank_project:{workspace_id}:Some(\"Gamma\")"),
        format!("add_existing_project:{workspace_id}:/tmp/imported"),
        format!("rename_project:{alpha}:Alpha renamed"),
        format!("update_project_order:{beta},{alpha},prj_created,prj_existing"),
        format!("update_project_expanded:{alpha}:false"),
        format!("get_project_git_status:{alpha}"),
        format!("init_project_git:{alpha}"),
        format!("get_project_instruction_summary:{alpha}"),
        format!("remove_project:{alpha}"),
    ] {
        assert!(
            calls.iter().any(|call| call == &expected),
            "expected call {expected}, got {calls:?}"
        );
    }

    assert!(
        app.state.projects.iter().all(|project| project.id != alpha),
        "removed project should leave local project list"
    );
    assert!(
        app.state
            .projects
            .iter()
            .any(|project| project.id == ProjectId::from_string("prj_created".to_string())),
        "created project should be inserted"
    );
    assert!(
        app.state
            .status_log
            .iter()
            .any(|entry| entry.message.contains("project instructions")),
        "instruction command should surface summary content in the status log"
    );
}

#[tokio::test]
async fn settings_utility_commands_call_facade_open_dir_methods() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::OpenConfigDir, Command::OpenSkillsDir],
    )
    .await;

    let calls = runtime.calls();
    assert!(
        calls.contains(&"open_config_dir".to_string()),
        "expected open_config_dir facade call; got {calls:?}"
    );
    assert!(
        calls.contains(&"open_skills_dir".to_string()),
        "expected open_skills_dir facade call; got {calls:?}"
    );
}
