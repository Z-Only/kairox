mod support;

use std::sync::Arc;

use agent_core::{ActivateSkillRequest, AppFacade, EventPayload, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_skills::{FileSkillRegistry, SkillRoot, SkillSourceKind};
use agent_store::{EventStore, SqliteEventStore};

use support::skills_helpers::{build_runtime_with_skill_registry, write_test_skill};

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
            approval_policy: None,
            sandbox_policy: None,
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
async fn active_skills_replay_from_events_after_runtime_recreation() {
    let skill_root = tempfile::tempdir().expect("skill root should be created");
    write_test_skill(
        skill_root.path(),
        "code-review",
        "Review code changes",
        "Use a careful review checklist.",
    );
    let registry = Arc::new(
        FileSkillRegistry::discover(vec![SkillRoot::new(
            SkillSourceKind::Workspace,
            skill_root.path(),
        )])
        .await
        .expect("skill registry should discover test skill"),
    );
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let runtime = LocalRuntime::new(store.clone(), FakeModelClient::new(vec!["ok".into()]))
        .with_skill_registry(registry.clone());

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
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            skill_id: "code-review".into(),
        })
        .await
        .expect("manual skill activation should succeed");

    let restored_runtime = LocalRuntime::new(store, FakeModelClient::new(vec!["ok".into()]))
        .with_skill_registry(registry);
    let active_skills = restored_runtime
        .list_active_skills(session_id)
        .await
        .expect("active skills should replay from SkillActivated events");

    assert_eq!(active_skills, vec![active_skill]);
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
            approval_policy: None,
            sandbox_policy: None,
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
