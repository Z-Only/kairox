use crate::facade_runtime::LocalRuntime;
use agent_core::SessionId;
use agent_store::EventStore;
use std::collections::HashMap;
use std::sync::Arc;

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    /// Inject the loaded `Config` so the runtime can resolve `ModelLimits`
    /// per session. Called by every production wiring site after `Config::load()`.
    pub fn with_config(mut self, config: Arc<agent_config::Config>) -> Self {
        self.config = config;
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
        let mut states = self.session_states.lock().await;
        let entry = states
            .entry(session_id.to_string())
            .or_insert_with(crate::session::SessionState::default);
        entry.model_limits = Some(limits);
    }

    pub(crate) async fn initialize_session_limits(
        &self,
        session_id: &SessionId,
        model_profile_alias: &str,
    ) {
        let profile_def = self
            .config
            .profiles
            .iter()
            .find(|(alias, _)| alias == model_profile_alias)
            .map(|(_, def)| def.clone());
        if let Some(def) = profile_def {
            let initial_limits = agent_config::resolve_limits(&def);
            self.set_session_limits(session_id, initial_limits.clone())
                .await;
            self.spawn_ollama_context_probe(session_id.clone(), model_profile_alias, &def);
        }
    }

    fn spawn_ollama_context_probe(
        &self,
        session_id: SessionId,
        profile_alias: &str,
        profile_def: &agent_config::ProfileDef,
    ) {
        if profile_def.provider != "ollama" {
            return;
        }

        if let Some(client) = self.ollama_clients.get(profile_alias).cloned() {
            let model_id = profile_def.model_id.clone();
            let session_id_for_probe = session_id.clone();
            let session_states = self.session_states.clone();
            tokio::spawn(async move {
                let probe = tokio::time::timeout(
                    std::time::Duration::from_secs(3),
                    client.probe_context_window(&model_id),
                )
                .await;
                if let Ok(Some(window)) = probe {
                    let mut states = session_states.lock().await;
                    if let Some(entry) = states.get_mut(session_id_for_probe.as_str()) {
                        if let Some(ref mut l) = entry.model_limits {
                            l.context_window = window;
                            l.source = agent_models::LimitSource::RuntimeProbe;
                        }
                    }
                }
            });
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
    /// Same-profile switches (alias equals the current profile) are a
    /// silent no-op — they return `Ok(())` without appending an event.
    pub async fn switch_model(
        &self,
        session_id: agent_core::SessionId,
        profile_alias: String,
    ) -> agent_core::Result<()> {
        // Validate alias exists in the loaded Config.
        let profile_def = self
            .config
            .profiles
            .iter()
            .find(|(alias, def)| alias == &profile_alias && def.enabled)
            .map(|(_, def)| def.clone())
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!("unknown model: {profile_alias}"))
            })?;

        // Resolve the session's current profile using the same helper
        // the agent loop uses — the two resolvers must never drift.
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        let from_profile = crate::agent_loop::latest_model_profile_for(&events);

        // Same-profile switch → silent no-op.
        if from_profile == profile_alias {
            return Ok(());
        }

        let workspace_id = events
            .first()
            .map(|e| e.workspace_id.clone())
            .ok_or_else(|| agent_core::CoreError::InvalidState("session has no events".into()))?;

        // Busy-gate mirrors `compact_session`: refuse switches while compaction
        // is mutating the same session state.
        {
            let states = self.session_states.lock().await;
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
                effective_at: chrono::Utc::now(),
                context_window: new_limits.context_window,
                output_limit: new_limits.output_limit,
                limit_source: limit_source_str.into(),
            },
        );
        crate::event_emitter::append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

        // Refresh cached limits so the next send_message's agent loop
        // doesn't re-derive from the old profile.
        self.set_session_limits(&session_id, new_limits.clone())
            .await;

        self.spawn_ollama_context_probe(session_id.clone(), &profile_alias, &profile_def);

        Ok(())
    }
}
