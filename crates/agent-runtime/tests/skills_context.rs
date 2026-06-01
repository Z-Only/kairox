mod support;

use std::sync::Arc;

use tokio::sync::Mutex as AsyncMutex;

use agent_core::{
    ActivateSkillRequest, AppFacade, ContextSource, SendMessageRequest, StartSessionRequest,
};
use agent_memory::{ContextAssembler, ContextRequest};
use agent_models::types::ServerTool;
use agent_runtime::LocalRuntime;
use agent_skills::{FileSkillRegistry, SkillRoot, SkillSourceKind};
use agent_store::SqliteEventStore;

use support::skills_helpers::{
    build_runtime_with_skill_registry, context_budget, write_test_skill, RecordingModelClient,
};

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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
async fn send_message_includes_profile_server_tools_in_model_request() {
    let mut config = agent_config::Config::defaults();
    let (_, fake_profile) = config
        .profiles
        .iter_mut()
        .find(|(alias, _)| alias == "fake")
        .expect("default fake profile should exist");
    fake_profile.server_tool_code_execution = Some(true);
    fake_profile.server_tool_web_search = Some(true);

    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let captured_requests = Arc::new(AsyncMutex::new(Vec::new()));
    let model = RecordingModelClient::new(captured_requests.clone());
    let runtime = LocalRuntime::new(store, model).with_config(Arc::new(config));

    let workspace = runtime
        .open_workspace(".".into())
        .await
        .expect("workspace should open");
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .expect("session should start");

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id,
            content: "use provider tools".into(),
            attachments: vec![],
        })
        .await
        .expect("send_message should complete");

    let requests = captured_requests.lock().await;
    let request = requests
        .first()
        .expect("model should receive one request after send_message");
    assert_eq!(
        request.server_tools,
        vec![
            ServerTool::CodeExecution,
            ServerTool::WebSearch {
                allowed_domains: Vec::new(),
                blocked_domains: Vec::new(),
                user_location: None,
            },
        ]
    );
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
