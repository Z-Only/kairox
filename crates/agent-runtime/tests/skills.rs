use std::sync::Arc;

use async_trait::async_trait;
use futures::{stream, stream::BoxStream};
use tokio::sync::Mutex as AsyncMutex;

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult,
    SkillInstallTarget, SkillUpdateState,
};
use agent_core::{
    ActivateSkillRequest, AppFacade, ContextSource, EventPayload, SendMessageRequest,
    StartSessionRequest,
};
use agent_memory::{ContextAssembler, ContextBudget, ContextRequest};
use agent_models::{FakeModelClient, ModelClient, ModelEvent, ModelRequest};
use agent_runtime::skill_package::{FakeSkillPackageManager, SkillPackageManager};
use agent_runtime::LocalRuntime;
use agent_skills::{FileSkillRegistry, SkillRoot, SkillSourceKind};
use agent_store::{EventStore, SqliteEventStore};

fn context_budget() -> ContextBudget {
    ContextBudget {
        context_window: 8_000,
        output_reservation: 1_000,
        source_caps: vec![],
    }
}

fn write_test_skill(root: &std::path::Path, name: &str, description: &str, body: &str) {
    let skill_directory = root.join(name);
    std::fs::create_dir_all(&skill_directory).expect("skill directory should be created");
    std::fs::write(
        skill_directory.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n{body}"),
    )
    .expect("skill file should be written");
}

async fn build_runtime_with_skill_registry(
    registry: Arc<dyn agent_skills::SkillRegistry>,
) -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    LocalRuntime::new(store, model).with_skill_registry(registry)
}

#[derive(Clone, Debug)]
struct RecordingModelClient {
    requests: Arc<AsyncMutex<Vec<ModelRequest>>>,
}

impl RecordingModelClient {
    fn new(requests: Arc<AsyncMutex<Vec<ModelRequest>>>) -> Self {
        Self { requests }
    }
}

#[async_trait]
impl ModelClient for RecordingModelClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        self.requests.lock().await.push(request);
        Ok(Box::pin(stream::iter(vec![
            Ok(ModelEvent::TokenDelta("ok".into())),
            Ok(ModelEvent::Completed { usage: None }),
        ])))
    }
}

#[tokio::test]
async fn manual_activation_lists_active_skills_for_that_session() {
    let skill_root = tempfile::tempdir().expect("skill root should be created");
    write_test_skill(
        skill_root.path(),
        "code-review",
        "Review code changes",
        "Use a careful review checklist.",
    );
    let registry = FileSkillRegistry::discover(vec![SkillRoot::new(
        SkillSourceKind::Workspace,
        skill_root.path(),
    )])
    .await
    .expect("skill registry should discover test skill");
    let runtime = build_runtime_with_skill_registry(Arc::new(registry)).await;

    let workspace = runtime
        .open_workspace(".".into())
        .await
        .expect("workspace should open");
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .expect("session should start");

    let active_skill = runtime
        .activate_skill(ActivateSkillRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            skill_id: "code-review".into(),
        })
        .await
        .expect("manual skill activation should succeed");

    assert_eq!(active_skill.skill_id, "code-review");
    assert_eq!(active_skill.name, "code-review");
    assert_eq!(active_skill.source, "workspace");
    assert_eq!(active_skill.activation_mode, "manual");

    let active_skills = runtime
        .list_active_skills(session_id.clone())
        .await
        .expect("active skills should be listed");
    assert_eq!(active_skills, vec![active_skill]);

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .expect("events should load");
    assert!(events.iter().any(|event| {
        matches!(
            &event.payload,
            EventPayload::SkillActivated { skill_id, .. } if skill_id == "code-review"
        )
    }));
}

#[tokio::test]
async fn repeated_skills_activation_does_not_emit_duplicate_skill_activated_events() {
    let skill_root = tempfile::tempdir().expect("skill root should be created");
    write_test_skill(
        skill_root.path(),
        "code-review",
        "Review code changes",
        "Use a careful review checklist.",
    );
    let registry = FileSkillRegistry::discover(vec![SkillRoot::new(
        SkillSourceKind::Workspace,
        skill_root.path(),
    )])
    .await
    .expect("skill registry should discover test skill");
    let runtime = build_runtime_with_skill_registry(Arc::new(registry)).await;

    let workspace = runtime
        .open_workspace(".".into())
        .await
        .expect("workspace should open");
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .expect("session should start");

    for _ in 0..2 {
        runtime
            .activate_skill(ActivateSkillRequest {
                workspace_id: workspace.workspace_id.clone(),
                session_id: session_id.clone(),
                skill_id: "code-review".into(),
            })
            .await
            .expect("manual skill activation should be idempotent");
    }

    let events = runtime
        .event_store_for_test()
        .load_session(&session_id)
        .await
        .expect("events should load");
    let skill_activated_count = events
        .iter()
        .filter(|event| {
            matches!(
                &event.payload,
                EventPayload::SkillActivated { skill_id, .. } if skill_id == "code-review"
            )
        })
        .count();
    assert_eq!(skill_activated_count, 1);
}

#[tokio::test]
async fn send_message_skips_missing_active_skills_documents() {
    let skill_root = tempfile::tempdir().expect("skill root should be created");
    write_test_skill(
        skill_root.path(),
        "code-review",
        "Review code changes",
        "Use a careful review checklist.",
    );
    let skill_file = skill_root.path().join("code-review").join("SKILL.md");
    let registry = FileSkillRegistry::discover(vec![SkillRoot::new(
        SkillSourceKind::Workspace,
        skill_root.path(),
    )])
    .await
    .expect("skill registry should discover test skill");
    let runtime = build_runtime_with_skill_registry(Arc::new(registry)).await;

    let workspace = runtime
        .open_workspace(".".into())
        .await
        .expect("workspace should open");
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .expect("session should start");
    runtime
        .activate_skill(ActivateSkillRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            skill_id: "code-review".into(),
        })
        .await
        .expect("manual skill activation should succeed");
    std::fs::remove_file(skill_file).expect("skill document should be removable");

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id,
            content: "continue despite missing skill document".into(),
            attachments: vec![],
        })
        .await
        .expect("missing active skill documents should not block send_message");
}

#[tokio::test]
async fn send_message_includes_active_skill_block_in_model_request() {
    let skill_root = tempfile::tempdir().expect("skill root should be created");
    write_test_skill(
        skill_root.path(),
        "code-review",
        "Review code changes",
        "Always inspect error handling before approving code.",
    );
    let registry = FileSkillRegistry::discover(vec![SkillRoot::new(
        SkillSourceKind::Workspace,
        skill_root.path(),
    )])
    .await
    .expect("skill registry should discover test skill");
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let captured_requests = Arc::new(AsyncMutex::new(Vec::new()));
    let model = RecordingModelClient::new(captured_requests.clone());
    let runtime = LocalRuntime::new(store, model).with_skill_registry(Arc::new(registry));

    let workspace = runtime
        .open_workspace(".".into())
        .await
        .expect("workspace should open");
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .expect("session should start");
    runtime
        .activate_skill(ActivateSkillRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            skill_id: "code-review".into(),
        })
        .await
        .expect("manual skill activation should succeed");

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id,
            content: "review this patch".into(),
            attachments: vec![],
        })
        .await
        .expect("send_message should complete");

    let requests = captured_requests.lock().await;
    let request = requests
        .first()
        .expect("model should receive one request after send_message");
    let request_text = std::iter::once(request.system_prompt.as_deref().unwrap_or_default())
        .chain(
            request
                .messages
                .iter()
                .map(|message| message.content.as_str()),
        )
        .collect::<Vec<_>>()
        .join("\n");

    assert!(request_text.contains("<active_skills>"));
    assert!(request_text.contains("<skill name=\"code-review\" source=\"workspace\">"));
    assert!(request_text.contains("Always inspect error handling before approving code."));
    assert!(request_text.contains("</active_skills>"));
}

#[tokio::test]
async fn context_assembler_injects_active_skills_after_system_prompt() {
    let assembler = ContextAssembler::new_standalone();
    let bundle = assembler
        .assemble(
            ContextRequest {
                system_prompt: Some("System prompt".into()),
                active_skills: vec![
                    "<skill name=\"code-review\" source=\"workspace\">\nReview carefully.\n</skill>"
                        .into(),
                ],
                user_request: "review this diff".into(),
                ..Default::default()
            },
            context_budget(),
        )
        .await;

    let combined_messages = bundle.messages.join("\n");
    assert!(combined_messages.contains("<active_skills>"));
    assert!(combined_messages.contains("<skill name=\"code-review\" source=\"workspace\">"));
    assert!(combined_messages.contains("</active_skills>"));

    let system_index = bundle
        .sources
        .iter()
        .position(|source| matches!(source, ContextSource::System))
        .expect("system source should be present");
    let skill_index = bundle
        .sources
        .iter()
        .position(|source| matches!(source, ContextSource::Skill))
        .expect("skill source should be present");
    let request_index = bundle
        .sources
        .iter()
        .position(|source| matches!(source, ContextSource::Request))
        .expect("request source should be present");

    assert!(system_index < skill_index);
    assert!(skill_index < request_index);
    assert!(bundle
        .usage
        .by_source
        .iter()
        .any(|(source, tokens)| matches!(source, ContextSource::Skill) && *tokens > 0));
}

// ── Skill package manager integration tests ──

fn remote_result(
    name: &str,
    description: &str,
    repository: &str,
    install_count: u64,
) -> RemoteSkillSearchResult {
    RemoteSkillSearchResult {
        name: name.into(),
        description: description.into(),
        repository: Some(repository.into()),
        install_count: Some(install_count),
        source_url: repository.into(),
        package: format!("@skills/{name}"),
    }
}

async fn build_runtime_with_package_manager(
    manager: Arc<dyn SkillPackageManager>,
) -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    LocalRuntime::new(store, model).with_skill_package_manager(manager)
}

#[tokio::test]
async fn search_remote_skills_delegates_to_package_manager() {
    let manager = Arc::new(FakeSkillPackageManager::default());
    let expected = remote_result(
        "code-review",
        "Review code changes",
        "obra/superpowers",
        1200,
    );
    manager.search_results.lock().await.push(expected.clone());

    let runtime = build_runtime_with_package_manager(manager.clone()).await;
    let results = runtime
        .search_remote_skills("review".into())
        .await
        .expect("search should succeed");

    assert_eq!(results, vec![expected]);
    assert_eq!(manager.search_queries.lock().await.as_slice(), ["review"]);
}

#[tokio::test]
async fn search_remote_skills_propagates_package_manager_error() {
    let manager = Arc::new(FakeSkillPackageManager::default());
    *manager.search_error.lock().await = Some("registry unavailable".to_string());

    let runtime = build_runtime_with_package_manager(manager).await;
    let error = runtime
        .search_remote_skills("review".into())
        .await
        .expect_err("search should fail");

    assert!(error.to_string().contains("registry unavailable"));
}

#[tokio::test]
async fn fake_package_manager_records_install_requests() {
    let manager = FakeSkillPackageManager::default();

    let registry_request = InstallRemoteSkillRequest {
        package: "@skills/code-review".into(),
        source: "registry".into(),
        target: SkillInstallTarget::Project,
        package_url: None,
    };
    let github_request = InstallGithubSkillRequest {
        source: "obra/superpowers".into(),
        target: SkillInstallTarget::User,
    };

    let project_root = tempfile::tempdir().expect("project root");
    let user_root = tempfile::tempdir().expect("user root");

    manager
        .install_from_registry(project_root.path(), &registry_request)
        .await
        .expect("registry install should succeed");
    manager
        .install_from_github(user_root.path(), &github_request)
        .await
        .expect("github install should succeed");

    assert_eq!(manager.registry_install_requests.lock().await.len(), 1);
    assert_eq!(
        manager.registry_install_requests.lock().await[0].package,
        "@skills/code-review"
    );
    assert_eq!(manager.github_install_requests.lock().await.len(), 1);
    assert_eq!(
        manager.github_install_requests.lock().await[0].source,
        "obra/superpowers"
    );
    // Verify install roots recorded
    assert_eq!(
        manager.registry_install_roots.lock().await.as_slice(),
        [project_root.path().to_path_buf()]
    );
    assert_eq!(
        manager.github_install_roots.lock().await.as_slice(),
        [user_root.path().to_path_buf()]
    );
}

#[tokio::test]
async fn fake_package_manager_check_updates_states() {
    let manager = FakeSkillPackageManager::default();

    // Default: Unknown
    assert_eq!(
        manager.check_updates("code-review").await.unwrap(),
        SkillUpdateState::Unknown
    );
    assert_eq!(
        manager.check_update_skill_ids.lock().await.as_slice(),
        ["code-review"]
    );

    // UpToDate
    *manager.check_updates_result.lock().await = SkillUpdateState::UpToDate;
    assert_eq!(
        manager.check_updates("code-review").await.unwrap(),
        SkillUpdateState::UpToDate
    );

    // UpdateAvailable
    *manager.check_updates_result.lock().await = SkillUpdateState::UpdateAvailable;
    assert_eq!(
        manager.check_updates("code-review").await.unwrap(),
        SkillUpdateState::UpdateAvailable
    );

    // Verify all calls recorded
    assert_eq!(manager.check_update_skill_ids.lock().await.len(), 3);
}

#[tokio::test]
async fn fake_package_manager_update_records_and_propagates_error() {
    let manager = FakeSkillPackageManager::default();

    // Successful update
    manager
        .update("code-review")
        .await
        .expect("update should succeed");
    assert_eq!(
        manager.update_skill_ids.lock().await.as_slice(),
        ["code-review"]
    );

    // Failed update
    *manager.update_error.lock().await = Some("network timeout".to_string());
    let error = manager
        .update("code-review")
        .await
        .expect_err("update should fail");
    assert!(error.to_string().contains("network timeout"));
    assert_eq!(manager.update_skill_ids.lock().await.len(), 2);
}

#[tokio::test]
async fn fake_package_manager_install_errors_propagate() {
    let manager = FakeSkillPackageManager::default();

    *manager.registry_install_error.lock().await = Some("registry refused install".to_string());
    *manager.github_install_error.lock().await = Some("repository not found".to_string());

    let registry_err = manager
        .install_from_registry(
            tempfile::tempdir().unwrap().path(),
            &InstallRemoteSkillRequest {
                package: "bad-pkg".into(),
                source: "registry".into(),
                target: SkillInstallTarget::User,
                package_url: None,
            },
        )
        .await
        .expect_err("registry install should fail");
    assert!(registry_err
        .to_string()
        .contains("registry refused install"));

    let github_err = manager
        .install_from_github(
            tempfile::tempdir().unwrap().path(),
            &InstallGithubSkillRequest {
                source: "bad/repo".into(),
                target: SkillInstallTarget::Project,
            },
        )
        .await
        .expect_err("github install should fail");
    assert!(github_err.to_string().contains("repository not found"));

    // Requests still recorded despite errors
    assert_eq!(manager.registry_install_requests.lock().await.len(), 1);
    assert_eq!(manager.github_install_requests.lock().await.len(), 1);
}

#[tokio::test]
async fn fake_package_manager_empty_search_returns_empty_vec() {
    let manager = FakeSkillPackageManager::default();
    // No search_results configured — defaults to empty

    let results = manager
        .search("nonexistent")
        .await
        .expect("search should not error for empty results");
    assert!(results.is_empty());
    assert_eq!(
        manager.search_queries.lock().await.as_slice(),
        ["nonexistent"]
    );
}

#[tokio::test]
async fn fake_package_manager_multiple_search_results() {
    let manager = FakeSkillPackageManager::default();
    let results = vec![
        remote_result("code-review", "Review code", "obra/cr", 1200),
        remote_result("brainstorming", "Brainstorm ideas", "obra/bs", 800),
        remote_result("debugging", "Debug issues", "obra/dbg", 500),
    ];
    *manager.search_results.lock().await = results.clone();

    let search_results = manager.search("obra").await.expect("search should succeed");
    assert_eq!(search_results, results);
    assert_eq!(search_results.len(), 3);
}

#[tokio::test]
async fn fake_package_manager_check_updates_error_propagates() {
    let manager = FakeSkillPackageManager::default();
    *manager.check_updates_error.lock().await = Some("unable to reach registry".to_string());

    let error = manager
        .check_updates("code-review")
        .await
        .expect_err("check updates should fail");
    assert!(error.to_string().contains("unable to reach registry"));
}
