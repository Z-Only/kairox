//! TUI App logic integration tests — agent settings commands and overlay.
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
use app_logic_common::TuiMcpFakeFacade;
use futures::StreamExt;
use std::sync::Arc;

#[tokio::test]
async fn tui_agent_settings_commands_call_facade_and_refresh_overlay() {
    use agent_core::facade::{AgentSettingsInput, AgentSettingsScope};
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::OpenAgentSettingsOverlay,
            Command::SaveAgentSettings {
                input: AgentSettingsInput {
                    scope: AgentSettingsScope::Project,
                    name: "planner".into(),
                    description: "Plans work".into(),
                    tools: vec!["search".into()],
                    model_profile: Some("reasoning".into()),
                    skills: vec!["kairox-dev-workflow".into()],
                    nickname_candidates: vec!["Planner".into()],
                    enabled: true,
                    instructions: "Break work into steps.".into(),
                },
            },
            Command::CopyAgentSettings {
                settings_id: "Builtin:worker".into(),
                scope: AgentSettingsScope::User,
            },
            Command::DeleteAgentSettings {
                settings_id: "User:planner".into(),
            },
        ],
    )
    .await;

    assert!(app.agent_overlay.is_visible());
    let calls = runtime.calls();
    for expected in [
        "list_agent_settings",
        "upsert_agent_settings:Project:planner",
        "copy_agent_settings:Builtin:worker:User",
        "delete_agent_settings:User:planner",
    ] {
        assert!(
            calls.iter().any(|call| call == expected),
            "expected call {expected}, got {calls:?}"
        );
    }
}
