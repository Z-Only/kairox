use super::*;
use agent_core::facade::SessionFacade;
use agent_core::{SessionId, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use std::collections::HashMap;
use std::sync::Arc;

const TEST_PROFILE_TOML: &str = r#"
[profiles.test-profile]
model_id = "gpt-4o-mini"
provider = "openai_compatible"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
context_window = 128000
output_limit = 16384

[profiles.second-profile]
model_id = "claude-sonnet-4-20250514"
provider = "anthropic"
base_url = "https://api.anthropic.com"
api_key_env = "ANTHROPIC_API_KEY"
context_window = 200000
output_limit = 8192
"#;

async fn build_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(store, model)
}

fn test_config() -> Arc<agent_config::Config> {
    let config = agent_config::load_from_str(TEST_PROFILE_TOML, "test.toml").unwrap();
    Arc::new(config)
}

/// Helper: build a runtime with config + a started session, returning (runtime, session_id).
async fn build_runtime_with_session(
    profile: &str,
) -> (LocalRuntime<SqliteEventStore, FakeModelClient>, SessionId) {
    let runtime = build_runtime().await.with_config(test_config());
    let workspace = SessionFacade::open_workspace(&runtime, "/tmp/test".into())
        .await
        .unwrap();
    let session_id = SessionFacade::start_session(
        &runtime,
        StartSessionRequest {
            workspace_id: workspace.workspace_id,
            model_profile: profile.into(),
            approval_policy: None,
            sandbox_policy: None,
        },
    )
    .await
    .unwrap();
    (runtime, session_id)
}

// ── with_config ─────────────────────────────────────────────────────

#[tokio::test]
async fn with_config_sets_config_snapshot() {
    let runtime = build_runtime().await;
    // Default config has no profiles.
    assert!(runtime.config().profiles.is_empty());

    let runtime = runtime.with_config(test_config());
    let snapshot = runtime.config();
    assert_eq!(snapshot.profiles.len(), 2);
    assert!(snapshot.profiles.iter().any(|(a, _)| a == "test-profile"));
    assert!(snapshot.profiles.iter().any(|(a, _)| a == "second-profile"));
}

// ── with_ollama_clients ─────────────────────────────────────────────

#[tokio::test]
async fn with_ollama_clients_stores_clients() {
    let runtime = build_runtime().await;
    assert!(runtime.ollama_clients.is_empty());

    let mut clients = HashMap::new();
    clients.insert(
        "ollama-local".into(),
        Arc::new(agent_models::OllamaClient::new(
            agent_models::OllamaConfig::default(),
        )),
    );
    let runtime = runtime.with_ollama_clients(clients);
    assert_eq!(runtime.ollama_clients.len(), 1);
    assert!(runtime.ollama_clients.contains_key("ollama-local"));
}

// ── set_session_limits ──────────────────────────────────────────────

#[tokio::test]
async fn set_session_limits_writes_and_reads() {
    let runtime = build_runtime().await;
    let session_id = SessionId::new();
    let limits = agent_models::ModelLimits {
        context_window: 128_000,
        output_limit: 16_384,
        source: agent_models::LimitSource::UserConfig,
    };

    runtime
        .set_session_limits(&session_id, limits.clone())
        .await;

    let states = runtime.session_states.lock().await;
    let entry = states.get(session_id.as_str()).expect("entry should exist");
    assert_eq!(entry.model_limits.as_ref().unwrap(), &limits);
}

#[tokio::test]
async fn set_session_limits_creates_default_entry_for_unknown_session() {
    let runtime = build_runtime().await;
    let session_id = SessionId::new();
    let limits = agent_models::ModelLimits {
        context_window: 64_000,
        output_limit: 4_096,
        source: agent_models::LimitSource::Fallback,
    };

    // No prior entry exists.
    assert!(runtime
        .session_states
        .lock()
        .await
        .get(session_id.as_str())
        .is_none());

    runtime
        .set_session_limits(&session_id, limits.clone())
        .await;

    let states = runtime.session_states.lock().await;
    let entry = states.get(session_id.as_str()).unwrap();
    assert_eq!(entry.model_limits.as_ref().unwrap(), &limits);
    // Default fields should be intact.
    assert!(!entry.compacting);
}

// ── initialize_session_limits ───────────────────────────────────────

#[tokio::test]
async fn initialize_session_limits_unknown_profile_is_noop() {
    let runtime = build_runtime().await.with_config(test_config());
    let session_id = SessionId::new();

    // Should not panic or write anything for a missing profile.
    runtime
        .initialize_session_limits(&session_id, "nonexistent-profile")
        .await;

    let states = runtime.session_states.lock().await;
    assert!(states.get(session_id.as_str()).is_none());
}

#[tokio::test]
async fn initialize_session_limits_known_profile_sets_limits() {
    let runtime = build_runtime().await.with_config(test_config());
    let session_id = SessionId::new();

    runtime
        .initialize_session_limits(&session_id, "test-profile")
        .await;

    let states = runtime.session_states.lock().await;
    let entry = states.get(session_id.as_str()).expect("entry should exist");
    let limits = entry.model_limits.as_ref().unwrap();
    assert_eq!(limits.context_window, 128_000);
    assert_eq!(limits.output_limit, 16_384);
    assert_eq!(limits.source, agent_models::LimitSource::UserConfig);
}

// ── switch_model ────────────────────────────────────────────────────

#[tokio::test]
async fn switch_model_unknown_alias_returns_invalid_state() {
    let (runtime, session_id) = build_runtime_with_session("test-profile").await;

    let result = runtime
        .switch_model(session_id, "no-such-profile".into(), None)
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        agent_core::CoreError::InvalidState(msg) => {
            assert!(msg.contains("unknown model"), "unexpected message: {msg}");
        }
        other => panic!("expected InvalidState, got: {other:?}"),
    }
}

#[tokio::test]
async fn switch_model_compacting_session_returns_session_busy() {
    let (runtime, session_id) = build_runtime_with_session("test-profile").await;

    // Mark session as compacting.
    runtime
        .session_states
        .lock()
        .await
        .entry(session_id.to_string())
        .or_default()
        .compacting = true;

    let result = runtime
        .switch_model(session_id.clone(), "second-profile".into(), None)
        .await;

    assert!(result.is_err());
    match result.unwrap_err() {
        agent_core::CoreError::SessionBusy {
            session_id: sid, ..
        } => {
            assert_eq!(sid, session_id.to_string());
        }
        other => panic!("expected SessionBusy, got: {other:?}"),
    }
}

#[tokio::test]
async fn switch_model_same_profile_no_reasoning_change_is_noop() {
    let (runtime, session_id) = build_runtime_with_session("test-profile").await;

    // Subscribe before the switch to check no events are emitted.
    let mut rx = runtime.event_tx.subscribe();

    let result = runtime
        .switch_model(session_id, "test-profile".into(), None)
        .await;

    assert!(result.is_ok());
    // No ModelProfileSwitched event should have been emitted.
    assert!(rx.try_recv().is_err());
}

#[tokio::test]
async fn switch_model_emits_model_profile_switched_event() {
    let (runtime, session_id) = build_runtime_with_session("test-profile").await;

    let mut rx = runtime.event_tx.subscribe();

    let result = runtime
        .switch_model(session_id, "second-profile".into(), None)
        .await;

    assert!(result.is_ok());

    // Drain events until we find a ModelProfileSwitched.
    let mut found = false;
    while let Ok(event) = rx.try_recv() {
        if let agent_core::EventPayload::ModelProfileSwitched {
            from_profile,
            to_profile,
            ..
        } = &event.payload
        {
            assert_eq!(from_profile, "test-profile");
            assert_eq!(to_profile, "second-profile");
            found = true;
            break;
        }
    }
    assert!(found, "expected ModelProfileSwitched event");
}

// ── set_session_limits_in_state (helper) ────────────────────────────

#[tokio::test]
async fn helper_set_session_limits_in_state_creates_new_entry() {
    let states: Arc<tokio::sync::Mutex<HashMap<String, crate::session::SessionState>>> =
        Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let session_id = SessionId::new();
    let limits = agent_models::ModelLimits {
        context_window: 100_000,
        output_limit: 8_000,
        source: agent_models::LimitSource::BuiltinRegistry,
    };

    set_session_limits_in_state(&states, &session_id, limits.clone()).await;

    let guard = states.lock().await;
    let entry = guard.get(session_id.as_str()).unwrap();
    assert_eq!(entry.model_limits.as_ref().unwrap(), &limits);
    // Newly created entry should have default compacting = false.
    assert!(!entry.compacting);
}

#[tokio::test]
async fn helper_set_session_limits_in_state_updates_existing_entry() {
    let states: Arc<tokio::sync::Mutex<HashMap<String, crate::session::SessionState>>> =
        Arc::new(tokio::sync::Mutex::new(HashMap::new()));
    let session_id = SessionId::new();

    // Pre-populate with a custom state.
    {
        let mut guard = states.lock().await;
        let existing = crate::session::SessionState {
            compacting: true,
            model_limits: Some(agent_models::ModelLimits {
                context_window: 50_000,
                output_limit: 2_000,
                source: agent_models::LimitSource::Fallback,
            }),
            ..Default::default()
        };
        guard.insert(session_id.to_string(), existing);
    }

    let new_limits = agent_models::ModelLimits {
        context_window: 200_000,
        output_limit: 32_000,
        source: agent_models::LimitSource::RuntimeProbe,
    };

    set_session_limits_in_state(&states, &session_id, new_limits.clone()).await;

    let guard = states.lock().await;
    let entry = guard.get(session_id.as_str()).unwrap();
    assert_eq!(entry.model_limits.as_ref().unwrap(), &new_limits);
    // compacting should be preserved — set_session_limits_in_state only touches model_limits.
    assert!(entry.compacting);
}
