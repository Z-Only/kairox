use crate::facade_runtime::LocalRuntime;
use agent_core::SessionId;
use agent_store::EventStore;
use std::collections::HashMap;
use std::sync::Arc;

type SessionStates = Arc<tokio::sync::Mutex<HashMap<String, crate::session::SessionState>>>;

struct SwitchModelOperation<S>
where
    S: EventStore + 'static,
{
    store: Arc<S>,
    event_tx: tokio::sync::broadcast::Sender<agent_core::DomainEvent>,
    session_states: SessionStates,
    config: Arc<agent_config::Config>,
    ollama_clients: HashMap<String, Arc<agent_models::OllamaClient>>,
}

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    /// Inject the loaded `Config` so the runtime can resolve `ModelLimits`
    /// per session. Called by every production wiring site after `Config::load()`.
    pub fn with_config(mut self, config: Arc<agent_config::Config>) -> Self {
        self.config = crate::facade_runtime::RuntimeConfig::from_arc(config);
        self
    }

    /// Register typed Ollama clients per profile alias. Called by the wiring
    /// code AFTER `build_router` so we retain the typed handle needed for
    /// `probe_context_window` (which `Arc<dyn ModelClient>` cannot expose).
    /// Idempotent — calling twice replaces the entries.
    pub fn with_ollama_clients(
        mut self,
        clients: HashMap<String, Arc<agent_models::OllamaClient>>,
    ) -> Self {
        self.ollama_clients = clients;
        self
    }

    /// Update the in-memory `SessionState` for `session_id` with newly
    /// resolved model limits. Inserts a default `SessionState` if missing.
    pub(crate) async fn set_session_limits(
        &self,
        session_id: &SessionId,
        limits: agent_models::ModelLimits,
    ) {
        set_session_limits_in_state(&self.session_states, session_id, limits).await;
    }

    pub(crate) async fn initialize_session_limits(
        &self,
        session_id: &SessionId,
        model_profile_alias: &str,
    ) {
        let config = self.config();
        let profile_def = config
            .profiles
            .iter()
            .find(|(alias, _)| alias == model_profile_alias)
            .map(|(_, def)| def.clone());
        if let Some(def) = profile_def {
            let initial_limits = agent_config::resolve_limits(&def);
            self.set_session_limits(session_id, initial_limits.clone())
                .await;
            spawn_ollama_context_probe_for(
                session_id.clone(),
                model_profile_alias,
                &def,
                &self.ollama_clients,
                self.session_states.clone(),
            );
        }
    }

    /// Switch the active model profile for an ongoing session.
    ///
    /// The switch takes effect at the next `send_message` call — any
    /// in-flight agent loop completes on the old profile end-to-end so
    /// provider-specific tool-call formats (Anthropic `tool_use` vs.
    /// OpenAI function-calling) don't get mixed mid-stream.
    ///
    /// Errors:
    /// - `CoreError::InvalidState` if the alias is unknown.
    /// - `CoreError::SessionBusy` if the session is currently compacting.
    ///
    /// Same-profile switches are a silent no-op unless they also change
    /// the profile's reasoning effort.
    pub async fn switch_model(
        &self,
        session_id: agent_core::SessionId,
        profile_alias: String,
        reasoning_effort: Option<String>,
    ) -> agent_core::Result<()> {
        let operation = SwitchModelOperation {
            store: self.store.clone(),
            event_tx: self.event_tx.clone(),
            session_states: self.session_states.clone(),
            config: self.config(),
            ollama_clients: self.ollama_clients.clone(),
        };
        let queued_session_id = session_id.clone();
        self.session_execution
            .run_operation(&queued_session_id, async move {
                operation
                    .execute(session_id, profile_alias, reasoning_effort)
                    .await
            })
            .await
    }
}

impl<S> SwitchModelOperation<S>
where
    S: EventStore + 'static,
{
    async fn execute(
        self,
        session_id: SessionId,
        profile_alias: String,
        reasoning_effort: Option<String>,
    ) -> agent_core::Result<()> {
        let Self {
            store,
            event_tx,
            session_states,
            config,
            ollama_clients,
        } = self;

        // Validate alias exists in the loaded Config.
        let profile_def = config
            .profiles
            .iter()
            .find(|(alias, def)| alias == &profile_alias && def.enabled)
            .map(|(_, def)| def.clone())
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!("unknown model: {profile_alias}"))
            })?;

        // Resolve the session's current profile using the same helper the agent
        // loop uses; the two resolvers must never drift.
        let events = store
            .load_session(&session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        let from_profile = crate::agent_loop::latest_model_profile_for(&events);
        let from_reasoning_effort = crate::agent_loop::latest_model_reasoning_effort_for(&events);
        let requested_reasoning_effort = if agent_config::profile_supports_reasoning(&profile_def) {
            reasoning_effort.filter(|effort| !effort.trim().is_empty())
        } else {
            None
        };

        // Same profile + unchanged/no requested reasoning means silent no-op.
        if from_profile == profile_alias
            && (requested_reasoning_effort.is_none()
                || requested_reasoning_effort == from_reasoning_effort)
        {
            return Ok(());
        }

        let workspace_id = events
            .first()
            .map(|e| e.workspace_id.clone())
            .ok_or_else(|| agent_core::CoreError::InvalidState("session has no events".into()))?;

        // Busy-gate mirrors `compact_session`: refuse switches while compaction
        // is mutating the same session state.
        {
            let states = session_states.lock().await;
            if let Some(entry) = states.get(&session_id.to_string()) {
                if entry.compacting {
                    return Err(agent_core::CoreError::SessionBusy {
                        session_id: session_id.to_string(),
                        reason: "context compaction in progress".into(),
                    });
                }
            }
        }

        // Resolve the new profile's limits (registry + user overrides).
        let new_limits = agent_config::resolve_limits(&profile_def);
        let limit_source_str = match new_limits.source {
            agent_models::LimitSource::UserConfig => "user_config",
            agent_models::LimitSource::BuiltinRegistry => "builtin_registry",
            agent_models::LimitSource::RuntimeProbe => "runtime_probe",
            agent_models::LimitSource::Fallback => "fallback",
        };

        let event = agent_core::DomainEvent::new(
            workspace_id,
            session_id.clone(),
            agent_core::AgentId::system(),
            agent_core::PrivacyClassification::MinimalTrace,
            agent_core::EventPayload::ModelProfileSwitched {
                from_profile,
                to_profile: profile_alias.clone(),
                reasoning_effort: requested_reasoning_effort,
                effective_at: chrono::Utc::now(),
                context_window: new_limits.context_window,
                output_limit: new_limits.output_limit,
                limit_source: limit_source_str.into(),
            },
        );
        crate::event_emitter::append_and_broadcast(&*store, &event_tx, &event).await?;

        store
            .update_session_model_profile(
                session_id.as_str(),
                &profile_alias,
                Some(profile_def.model_id.as_str()),
                Some(profile_def.provider.as_str()),
            )
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;

        // Refresh cached limits so the next send_message's agent loop does not
        // re-derive from the old profile.
        set_session_limits_in_state(&session_states, &session_id, new_limits).await;

        spawn_ollama_context_probe_for(
            session_id,
            &profile_alias,
            &profile_def,
            &ollama_clients,
            session_states,
        );

        Ok(())
    }
}

async fn set_session_limits_in_state(
    session_states: &SessionStates,
    session_id: &SessionId,
    limits: agent_models::ModelLimits,
) {
    let mut states = session_states.lock().await;
    let entry = states
        .entry(session_id.to_string())
        .or_insert_with(crate::session::SessionState::default);
    entry.model_limits = Some(limits);
}

fn spawn_ollama_context_probe_for(
    session_id: SessionId,
    profile_alias: &str,
    profile_def: &agent_config::ProfileDef,
    ollama_clients: &HashMap<String, Arc<agent_models::OllamaClient>>,
    session_states: SessionStates,
) {
    if profile_def.provider != "ollama" {
        return;
    }

    if let Some(client) = ollama_clients.get(profile_alias).cloned() {
        let model_id = profile_def.model_id.clone();
        let session_id_for_probe = session_id.clone();
        tokio::spawn(async move {
            let probe = tokio::time::timeout(
                std::time::Duration::from_secs(3),
                client.probe_context_window(&model_id),
            )
            .await;
            if let Ok(Some(window)) = probe {
                let mut states = session_states.lock().await;
                if let Some(entry) = states.get_mut(session_id_for_probe.as_str()) {
                    if let Some(ref mut limits) = entry.model_limits {
                        limits.context_window = window;
                        limits.source = agent_models::LimitSource::RuntimeProbe;
                    }
                }
            }
        });
    }
}
