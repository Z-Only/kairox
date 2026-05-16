use std::sync::Arc;

use async_trait::async_trait;
use futures::{stream, stream::BoxStream};
use tokio::sync::Mutex as AsyncMutex;

use agent_core::{
    ActivateSkillRequest, AppFacade, ContextSource, EventPayload, SendMessageRequest,
    StartSessionRequest,
};
use agent_memory::{ContextAssembler, ContextBudget, ContextRequest};
use agent_models::{FakeModelClient, ModelClient, ModelEvent, ModelRequest};
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
