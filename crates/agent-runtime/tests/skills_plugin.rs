mod support;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use tokio::sync::Mutex as AsyncMutex;

use agent_core::{ActivateSkillRequest, AppFacade, SendMessageRequest, StartSessionRequest};
use agent_runtime::LocalRuntime;
use agent_skills::{FileSkillRegistry, SkillRoot, SkillSourceKind};
use agent_store::SqliteEventStore;

use support::skills_helpers::{
    build_runtime_with_skill_registry, write_plugin_skill, write_test_skill, RecordingModelClient,
    ToggleSkillRegistry,
};

#[tokio::test]
async fn activate_plugin_skill_with_namespaced_id() {
    let skill_root = tempfile::tempdir().expect("skill root");
    write_plugin_skill(
        skill_root.path(),
        "my-plugin",
        "review",
        "Plugin review skill",
        "Review carefully.",
    );

    let registry = FileSkillRegistry::discover(vec![SkillRoot::with_namespace(
        SkillSourceKind::Plugin,
        skill_root.path(),
        "my-plugin",
    )])
    .await
    .expect("registry should discover plugin skill");
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

    let active_skill = runtime
        .activate_skill(ActivateSkillRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            skill_id: "my-plugin:review".into(),
        })
        .await
        .expect("plugin skill activation should succeed");

    assert_eq!(active_skill.skill_id, "my-plugin:review");
    assert_eq!(active_skill.name, "review");
    assert_eq!(active_skill.source, "plugin");

    let active_skills = runtime
        .list_active_skills(session_id.clone())
        .await
        .expect("active skills should be listed");
    assert_eq!(active_skills, vec![active_skill]);
}

#[tokio::test]
async fn disabled_plugin_skill_does_not_reactivate_from_stale_session_state() {
    let enabled = Arc::new(AtomicBool::new(true));
    let registry = ToggleSkillRegistry::plugin_review(enabled.clone());
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
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            skill_id: "my-plugin:review".into(),
        })
        .await
        .expect("plugin skill activation should succeed");

    enabled.store(false, Ordering::SeqCst);
    let active_after_disable = runtime
        .list_active_skills(session_id.clone())
        .await
        .expect("active skills should be listed after disable");
    assert!(
        active_after_disable.is_empty(),
        "disabled plugin skill should be hidden from active skills"
    );

    enabled.store(true, Ordering::SeqCst);
    let active_after_reenable = runtime
        .list_active_skills(session_id)
        .await
        .expect("active skills should be listed after re-enable");
    assert!(
        active_after_reenable.is_empty(),
        "stale active skill state should be pruned when plugin discovery drops it"
    );
}

#[tokio::test]
async fn plugin_skill_block_appears_in_model_request() {
    let skill_root = tempfile::tempdir().expect("skill root");
    write_plugin_skill(
        skill_root.path(),
        "my-plugin",
        "review",
        "Plugin review skill",
        "Always check error handling.",
    );

    let registry = FileSkillRegistry::discover(vec![SkillRoot::with_namespace(
        SkillSourceKind::Plugin,
        skill_root.path(),
        "my-plugin",
    )])
    .await
    .expect("registry should discover plugin skill");

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
            skill_id: "my-plugin:review".into(),
        })
        .await
        .expect("plugin skill activation should succeed");

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
    let request = requests.first().expect("model should receive one request");
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
    assert!(
        request_text.contains("<skill name=\"review\" source=\"plugin\">"),
        "expected plugin skill block with display name and plugin source, got:\n{request_text}"
    );
    assert!(request_text.contains("Always check error handling."));
    assert!(request_text.contains("</active_skills>"));
}

#[tokio::test]
async fn disabled_plugin_skill_is_pruned_before_model_request_blocks() {
    let enabled = Arc::new(AtomicBool::new(true));
    let registry = ToggleSkillRegistry::plugin_review(enabled.clone());

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
            skill_id: "my-plugin:review".into(),
        })
        .await
        .expect("plugin skill activation should succeed");

    enabled.store(false, Ordering::SeqCst);
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "plugin disabled".into(),
            attachments: vec![],
        })
        .await
        .expect("disabled plugin skill should not block send_message");

    enabled.store(true, Ordering::SeqCst);
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id,
            content: "plugin re-enabled".into(),
            attachments: vec![],
        })
        .await
        .expect("re-enabled plugin should not reactivate stale skill");

    let requests = captured_requests.lock().await;
    assert_eq!(requests.len(), 2);
    for request in requests.iter() {
        let request_text = std::iter::once(request.system_prompt.as_deref().unwrap_or_default())
            .chain(
                request
                    .messages
                    .iter()
                    .map(|message| message.content.as_str()),
            )
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            !request_text.contains("<active_skills>"),
            "disabled plugin skill should be pruned before future model requests, got:\n{request_text}"
        );
    }
}

#[tokio::test]
async fn activate_plugin_skill_without_namespace_qualifier_fails() {
    let skill_root = tempfile::tempdir().expect("skill root");
    write_plugin_skill(
        skill_root.path(),
        "my-plugin",
        "review",
        "Plugin review skill",
        "Review carefully.",
    );

    let registry = FileSkillRegistry::discover(vec![SkillRoot::with_namespace(
        SkillSourceKind::Plugin,
        skill_root.path(),
        "my-plugin",
    )])
    .await
    .expect("registry should discover plugin skill");
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

    // Activating just "review" without the namespace should fail
    let error = runtime
        .activate_skill(ActivateSkillRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            skill_id: "review".into(),
        })
        .await
        .expect_err("activation without namespace should fail");
    assert!(
        error.to_string().contains("review"),
        "error should mention the skill id: {error}"
    );
}

#[tokio::test]
async fn list_skills_includes_plugin_skills_with_namespaced_ids() {
    let plugin_root = tempfile::tempdir().expect("plugin root");
    let workspace_root = tempfile::tempdir().expect("workspace root");

    write_plugin_skill(
        plugin_root.path(),
        "my-plugin",
        "review",
        "Plugin review skill",
        "Plugin body.",
    );
    write_test_skill(
        workspace_root.path(),
        "review",
        "Workspace review skill",
        "Workspace body.",
    );

    let registry = FileSkillRegistry::discover(vec![
        SkillRoot::with_namespace(SkillSourceKind::Plugin, plugin_root.path(), "my-plugin"),
        SkillRoot::new(SkillSourceKind::Workspace, workspace_root.path()),
    ])
    .await
    .expect("registry should discover skills");
    let runtime = build_runtime_with_skill_registry(Arc::new(registry)).await;

    let _workspace = runtime
        .open_workspace(".".into())
        .await
        .expect("workspace should open");

    let skills = runtime
        .list_skills()
        .await
        .expect("skills should be listed");

    // Two distinct skills: workspace "review" and plugin "my-plugin:review"
    assert_eq!(skills.len(), 2);
    let ids: Vec<&str> = skills.iter().map(|s| s.id.as_str()).collect();
    assert!(ids.contains(&"review"));
    assert!(ids.contains(&"my-plugin:review"));

    let plugin_skill = skills
        .iter()
        .find(|s| s.id == "my-plugin:review")
        .expect("plugin skill");
    assert_eq!(plugin_skill.source, "plugin");
    assert!(plugin_skill.valid);

    let workspace_skill = skills
        .iter()
        .find(|s| s.id == "review")
        .expect("workspace skill");
    assert_eq!(workspace_skill.source, "workspace");
    assert!(workspace_skill.valid);
}

#[tokio::test]
async fn same_name_workspace_and_plugin_skills_activate_independently() {
    let plugin_root = tempfile::tempdir().expect("plugin root");
    let workspace_root = tempfile::tempdir().expect("workspace root");

    write_plugin_skill(
        plugin_root.path(),
        "my-plugin",
        "review",
        "Plugin review skill",
        "Plugin body.",
    );
    write_test_skill(
        workspace_root.path(),
        "review",
        "Workspace review skill",
        "Workspace body.",
    );

    let registry = FileSkillRegistry::discover(vec![
        SkillRoot::with_namespace(SkillSourceKind::Plugin, plugin_root.path(), "my-plugin"),
        SkillRoot::new(SkillSourceKind::Workspace, workspace_root.path()),
    ])
    .await
    .expect("registry should discover skills");
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

    let workspace_skill = runtime
        .activate_skill(ActivateSkillRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            skill_id: "review".into(),
        })
        .await
        .expect("workspace skill activation should succeed");
    let plugin_skill = runtime
        .activate_skill(ActivateSkillRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            skill_id: "my-plugin:review".into(),
        })
        .await
        .expect("plugin skill activation should succeed");

    assert_eq!(workspace_skill.skill_id, "review");
    assert_eq!(workspace_skill.name, "review");
    assert_eq!(workspace_skill.source, "workspace");
    assert_eq!(plugin_skill.skill_id, "my-plugin:review");
    assert_eq!(plugin_skill.name, "review");
    assert_eq!(plugin_skill.source, "plugin");

    let active_skills = runtime
        .list_active_skills(session_id)
        .await
        .expect("active skills should be listed");
    assert_eq!(active_skills, vec![workspace_skill, plugin_skill]);
}

#[tokio::test]
async fn get_skill_detail_returns_plugin_skill_body() {
    let skill_root = tempfile::tempdir().expect("skill root");
    write_plugin_skill(
        skill_root.path(),
        "my-plugin",
        "review",
        "Plugin review skill",
        "Detailed review instructions.",
    );

    let registry = FileSkillRegistry::discover(vec![SkillRoot::with_namespace(
        SkillSourceKind::Plugin,
        skill_root.path(),
        "my-plugin",
    )])
    .await
    .expect("registry should discover plugin skill");
    let runtime = build_runtime_with_skill_registry(Arc::new(registry)).await;

    let _workspace = runtime
        .open_workspace(".".into())
        .await
        .expect("workspace should open");

    let detail = runtime
        .get_skill("my-plugin:review".into())
        .await
        .expect("plugin skill detail should be fetched")
        .expect("plugin skill should exist");

    assert_eq!(detail.view.id, "my-plugin:review");
    assert_eq!(detail.view.source, "plugin");
    assert_eq!(detail.body_markdown, "Detailed review instructions.");
}
