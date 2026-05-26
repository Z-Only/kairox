//! Verifies that the agent loop emits `ContextAssembled { usage }` with
//! `total_tokens <= budget_tokens` and that the `usage.by_source`
//! breakdown contains the expected categories.

use agent_core::{
    AppFacade, EventPayload, SendMessageRequest, SessionId, StartSessionRequest, WorkspaceId,
};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::HashMap;
use std::sync::Arc;

async fn build_test_runtime() -> Arc<LocalRuntime<SqliteEventStore, FakeModelClient>> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let _ = pool; // pool no longer needed; use the canonical helper.
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["ok".into(); 16]);
    Arc::new(LocalRuntime::new(store, model))
}

async fn load_events(
    runtime: &LocalRuntime<SqliteEventStore, FakeModelClient>,
    session_id: &SessionId,
) -> Vec<agent_core::DomainEvent> {
    use agent_store::EventStore;
    runtime
        .event_store_for_test()
        .load_session(session_id)
        .await
        .unwrap()
}

#[tokio::test]
async fn context_assembled_event_emitted_with_budget_respected() {
    let runtime = build_test_runtime().await;

    let ws_info = runtime.open_workspace(".".into()).await.unwrap();
    let workspace_id: WorkspaceId = ws_info.workspace_id;
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    for i in 0..5 {
        runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                content: format!("turn {} please", i),
                attachments: vec![],
            })
            .await
            .unwrap();
    }

    let events = load_events(&runtime, &session_id).await;
    let assembled: Vec<_> = events
        .iter()
        .filter_map(|e| match &e.payload {
            EventPayload::ContextAssembled { usage } => Some(usage),
            _ => None,
        })
        .collect();

    assert!(
        !assembled.is_empty(),
        "expected at least one ContextAssembled event"
    );
    for usage in &assembled {
        assert!(
            usage.total_tokens <= usage.budget_tokens,
            "ContextAssembled.total_tokens ({}) exceeded budget_tokens ({})",
            usage.total_tokens,
            usage.budget_tokens
        );
        assert_eq!(usage.estimator, "cl100k_base");
        assert_eq!(usage.context_window, 4_096); // FALLBACK_FAKE from model_registry
        assert!(usage
            .by_source
            .iter()
            .any(|(s, n)| matches!(s, agent_core::ContextSource::System) && *n > 0));
    }
}

/// Verifies that when a session uses an Ollama profile, the runtime fires
/// the `probe_context_window` request on `start_session` and the next
/// `ContextAssembled` event reflects the probed value (NOT the registry
/// fallback nor the TOML override).
#[tokio::test]
async fn ollama_session_uses_probed_context_window() {
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    // 1. Mock Ollama server: /api/show returns a 32k window via
    //    `model_info."llama.context_length"` (the field name used by
    //    `probe_context_window` in `crates/agent-models/src/ollama.rs`).
    let mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/show"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "model_info": { "llama.context_length": 32_768_u64 }
        })))
        .mount(&mock)
        .await;

    // 2. Build runtime in-line — `build_test_runtime` doesn't know about
    //    Ollama, so we mirror the production wiring (router + ollama_clients
    //    + config) here.
    let store = SqliteEventStore::in_memory().await.unwrap();
    // FakeModelClient handles the actual chat completion — the Ollama probe
    // is independent of the model client used for `send_message`.
    let model = FakeModelClient::new(vec!["ok".into(); 8]);

    let config_toml = format!(
        r#"
[profiles.ollama-test]
provider = "ollama"
model_id = "llama3"
base_url = "{}"
"#,
        mock.uri()
    );
    let config = Arc::new(agent_config::load_from_str(&config_toml, "test-inline.toml").unwrap());

    let ollama_client = Arc::new(agent_models::OllamaClient::new(
        agent_models::OllamaConfig {
            base_url: mock.uri(),
            default_model: "llama3".into(),
            context_window: 8_192, // fallback — should be overridden by probe
        },
    ));
    let mut clients = HashMap::new();
    clients.insert("ollama-test".to_string(), ollama_client);

    let runtime = Arc::new(
        LocalRuntime::new(store, model)
            .with_config(config)
            .with_ollama_clients(clients),
    );

    // 3. Open workspace + start session — probe fires here as a tokio::spawn.
    let ws_info = runtime.open_workspace(".".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: ws_info.workspace_id.clone(),
            model_profile: "ollama-test".into(),

            permission_mode: None,
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    // 4. Wait for the spawned probe to land. The probe itself is bounded to
    //    3s, so poll the actual session limit instead of assuming a fixed
    //    sleep is enough on every CI machine.
    let probe_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(4);
    loop {
        let context_window = {
            let states = runtime.session_states_for_test().lock().await;
            states
                .get(session_id.as_str())
                .and_then(|state| state.model_limits.as_ref())
                .map(|limits| limits.context_window)
        };
        if context_window == Some(32_768) {
            break;
        }
        if tokio::time::Instant::now() >= probe_deadline {
            panic!(
                "timed out waiting for Ollama probe to override context window; last value: {:?}",
                context_window
            );
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    // 5. Send one message → ContextAssembled is emitted with the probed window.
    runtime
        .send_message(SendMessageRequest {
            workspace_id: ws_info.workspace_id,
            session_id: session_id.clone(),
            content: "hi".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let events = load_events(&runtime, &session_id).await;
    let usage = events
        .iter()
        .find_map(|e| match &e.payload {
            EventPayload::ContextAssembled { usage } => Some(usage),
            _ => None,
        })
        .expect("ContextAssembled emitted");
    assert_eq!(
        usage.context_window, 32_768,
        "Ollama probe should have overridden the fallback (got {})",
        usage.context_window
    );

    // Avoid "unused" warnings when the pool helper is no-op'd.
    let _ = SqlitePoolOptions::new();
}
