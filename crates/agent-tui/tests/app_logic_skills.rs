//! TUI App logic integration tests — skills commands, sources, catalog, and settings.
//!
//! Split from the former `app_logic.rs`. Shared helpers live in
//! `app_logic_common`.

#![allow(unused_imports)]

mod app_logic_common;

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_skills::{FileSkillRegistry, SkillRoot, SkillSourceKind};
use agent_store::SqliteEventStore;
use app_logic_common::{
    test_project, unique_temp_dir, write_test_skill, FakeInstallResult, TuiMcpFakeFacade,
};
use futures::StreamExt;
use std::sync::Arc;

#[tokio::test]
async fn tui_skill_commands_call_facade_and_render_visible_messages() {
    use agent_core::{EventPayload, SessionId, WorkspaceId};
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let skill_root = std::env::temp_dir().join(format!(
        "kairox-tui-skill-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos()
    ));
    std::fs::create_dir_all(&skill_root).expect("skill root should be created");
    write_test_skill(
        &skill_root,
        "test-driven-rust",
        "Use when implementing Rust changes with test-first development.",
        "# Test-driven Rust\n\nWrite a failing test first.\n",
    );
    let registry = FileSkillRegistry::discover(vec![SkillRoot::new(
        SkillSourceKind::Workspace,
        &skill_root,
    )])
    .await
    .expect("skill registry should discover test skill");
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    let runtime = Arc::new(LocalRuntime::new(store, model).with_skill_registry(Arc::new(registry)));

    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let mut app = App::new("fake", workspace_id.clone());
    app.current_session_id = Some(session_id.clone());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::ListSkills,
            Command::ShowSkill {
                skill_id: "test-driven-rust".into(),
            },
            Command::ActivateSkill {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                skill_id: "test-driven-rust".into(),
            },
            Command::DeactivateSkill {
                workspace_id,
                session_id: session_id.clone(),
                skill_id: "test-driven-rust".into(),
            },
        ],
    )
    .await;

    let status_messages: Vec<&str> = app
        .state
        .status_log
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("test-driven-rust")),
        "expected a skill list/detail status; got {status_messages:?}"
    );
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("activated test-driven-rust")),
        "expected activation confirmation status; got {status_messages:?}"
    );
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("deactivated test-driven-rust")),
        "expected deactivation confirmation status; got {status_messages:?}"
    );

    let trace = runtime
        .get_trace(session_id)
        .await
        .expect("skill commands should write trace events");
    assert!(
        trace.iter().any(|entry| {
            matches!(
                &entry.event.payload,
                EventPayload::SkillActivated { skill_id, .. } if skill_id == "test-driven-rust"
            )
        }),
        "expected SkillActivated trace event; got {trace:?}"
    );
    assert!(
        trace.iter().any(|entry| {
            matches!(
                &entry.event.payload,
                EventPayload::SkillDeactivated { skill_id, .. } if skill_id == "test-driven-rust"
            )
        }),
        "expected SkillDeactivated trace event; got {trace:?}"
    );

    std::fs::remove_dir_all(skill_root).expect("test skill root should be cleaned up");
}

#[tokio::test]
async fn tui_skill_source_commands_call_facade_and_refresh_overlay() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenSkillsOverlay]).await;

    assert!(app.skills_overlay.is_visible());
    let calls = runtime.calls();
    assert!(
        calls.iter().any(|call| call == "list_skill_settings"),
        "expected settings list call, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|call| call.starts_with("list_skill_catalog")),
        "expected skill catalog list call, got {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call == "list_skill_sources"),
        "expected skill sources call, got {calls:?}"
    );

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::AddSkillSource {
                config: agent_core::facade::SkillSourceView {
                    id: "corp".into(),
                    display_name: "Corporate Skills".into(),
                    kind: "skillhub".into(),
                    url: "https://skills.example.com".into(),
                    search_template: "/api/skills?keyword={{query}}".into(),
                    download_template: "/api/v1/download?slug={{slug}}".into(),
                    list_template: None,
                    detail_template: None,
                    field_mapping: agent_core::facade::SkillFieldMappingView::default(),
                    enabled: true,
                    priority: 100,
                    cache_ttl_seconds: 900,
                    last_error: None,
                },
            },
            Command::RemoveSkillSource {
                source_id: "corp".into(),
            },
            Command::SetSkillSourceEnabled {
                source_id: "skillhub".into(),
                enabled: false,
            },
        ],
    )
    .await;

    let calls = runtime.calls();
    for expected in [
        "add_skill_source:corp",
        "remove_skill_source:corp",
        "set_skill_source_enabled:skillhub:false",
    ] {
        assert!(
            calls.iter().any(|call| call == expected),
            "expected call {expected}, got {calls:?}"
        );
    }
    assert!(
        calls
            .iter()
            .filter(|call| call.as_str() == "list_skill_sources")
            .count()
            >= 4,
        "expected source mutations to refresh overlay, got {calls:?}"
    );
}

#[tokio::test]
async fn tui_skill_catalog_overlay_queries_include_keyword_and_sources() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenSkillsOverlay]).await;
    assert!(app.skills_overlay.is_visible());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::ListSkillCatalog {
            keyword: Some("review".into()),
            sources: Some(vec!["skillhub".into()]),
        }],
    )
    .await;

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::RefreshSkillCatalog {
            keyword: Some("docs".into()),
            sources: Some(vec!["skillhub".into()]),
        }],
    )
    .await;

    let calls = runtime.calls();
    assert!(
        calls.iter().any(
            |call| call
                == "list_skill_catalog:Some(\"review\"):Some([\"skillhub\"]):Some(50)"
        ),
        "expected filtered overlay list call, got {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call == "refresh_skill_catalog"),
        "expected catalog refresh call, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|call| call == "list_skill_catalog:Some(\"docs\"):Some([\"skillhub\"]):Some(50)"),
        "expected refresh to rerun filtered overlay query, got {calls:?}"
    );
}

#[tokio::test]
async fn tui_skill_catalog_settings_commands_call_facade_and_render_visible_messages() {
    use agent_core::facade::{InstallRemoteSkillRequest, SkillInstallTarget, SkillUpdateState};
    use agent_core::WorkspaceId;
    use agent_runtime::skill_package::FakeSkillPackageManager;
    use agent_runtime::skill_settings::SkillSettingsRoots;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let temp_root = std::env::temp_dir().join(format!(
        "kairox-tui-skill-settings-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos()
    ));
    let user_root = temp_root.join("user-skills");
    let catalog_root = temp_root.join("catalog");
    std::fs::create_dir_all(&user_root).expect("user skill root should be created");
    std::fs::create_dir_all(&catalog_root).expect("catalog root should be created");
    std::fs::write(
        catalog_root.join("skill_sources.toml"),
        r#"
[[skill_sources]]
id = "skillhub"
display_name = "SkillHub"
kind = "skillhub"
url = "https://api.skillhub.cn"
search_template = "/api/skills?keyword={{query}}"
download_template = "/api/v1/download?slug={{slug}}"
enabled = false
priority = 1
cache_ttl_seconds = 900
"#,
    )
    .expect("disabled catalog source should be written");
    write_test_skill(
        &user_root,
        "review",
        "Review code changes.",
        "# Review\n\nReview code carefully.\n",
    );

    let manager = Arc::new(FakeSkillPackageManager::default());
    *manager.check_updates_result.lock().await = SkillUpdateState::UpToDate;
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    let runtime = Arc::new(
        LocalRuntime::new(store, model)
            .with_skill_package_manager(manager.clone())
            .with_skill_settings_roots(SkillSettingsRoots {
                workspace_root: None,
                user_root: Some(user_root.clone()),
                builtin_root: None,
                plugin_roots: Vec::new(),
            })
            .with_skill_catalog(Some(catalog_root)),
    );

    let workspace_id = WorkspaceId::new();
    let mut app = App::new("fake", workspace_id);

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::ListSkillCatalog {
                keyword: Some("review".into()),
                sources: None,
            },
            Command::InstallRemoteSkill {
                request: InstallRemoteSkillRequest {
                    package: "@skills/review".into(),
                    source: "skillhub".into(),
                    target: SkillInstallTarget::User,
                    package_url: Some("https://example.test/review.zip".into()),
                },
            },
            Command::UpdateSkillSettings {
                skill_id: "review".into(),
            },
            Command::DeleteSkillSettings {
                skill_id: "review".into(),
            },
        ],
    )
    .await;

    let status_messages: Vec<&str> = app
        .state
        .status_log
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("No catalog skills found for review")),
        "expected catalog empty-state status; got {status_messages:?}"
    );
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("installed skill review")),
        "expected install confirmation status; got {status_messages:?}"
    );
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("updated skill review")),
        "expected update confirmation status; got {status_messages:?}"
    );
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("deleted skill review")),
        "expected delete confirmation status; got {status_messages:?}"
    );
    assert_eq!(manager.registry_install_requests.lock().await.len(), 1);
    assert_eq!(
        manager.registry_install_requests.lock().await[0].package,
        "@skills/review"
    );
    assert_eq!(manager.update_skill_ids.lock().await.as_slice(), ["review"]);
    assert!(
        !user_root.join("review").exists(),
        "delete command should remove the user skill directory"
    );

    std::fs::remove_dir_all(temp_root).expect("test skill settings root should be cleaned up");
}
