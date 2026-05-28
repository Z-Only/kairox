use agent_core::SessionId;
use agent_store::EventStore;

use super::local_runtime::LocalRuntime;

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    /// Trigger a compaction pass for `session_id`. Queues behind any active
    /// session actor turn and blocks until the chain completes (success or
    /// fallback). Returns `Err(SessionBusy)` if a compaction is already
    /// running for the same session.
    ///
    /// This is the inherent method; P3 will surface it via the `AppFacade`
    /// trait once the GUI/TUI commands wire to it.
    pub async fn compact_session(
        &self,
        session_id: SessionId,
        reason: agent_core::CompactionReason,
    ) -> agent_core::Result<()> {
        // Resolve the workspace_id from the first event of the session.
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        let workspace_id = events
            .first()
            .map(|e| e.workspace_id.clone())
            .ok_or_else(|| agent_core::CoreError::InvalidState("session has no events".into()))?;

        // Pre-check the busy gate so we can surface SessionBusy upfront
        // (the orchestrator silently no-ops when already compacting).
        {
            let states = self.session_states.lock().await;
            if let Some(entry) = states.get(&session_id.to_string()) {
                if entry.compacting {
                    return Err(agent_core::CoreError::SessionBusy {
                        session_id: session_id.to_string(),
                        reason: "compaction already running".into(),
                    });
                }
            }
        }

        // Pick the profile alias for the summarisation call:
        // ContextPolicy.compactor_profile takes priority; otherwise fall
        // back to the session's current profile (from SessionInitialized).
        let profile_alias = self
            .config
            .context
            .compactor_profile
            .clone()
            .unwrap_or_else(|| {
                events
                    .iter()
                    .find_map(|e| match &e.payload {
                        agent_core::EventPayload::SessionInitialized { model_profile } => {
                            Some(model_profile.clone())
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| "fake".to_string())
            });

        let store = self.store.clone();
        let event_tx = self.event_tx.clone();
        let model = self.model.clone();
        let session_states = self.session_states.clone();
        let operation_session_id = session_id.clone();

        self.session_execution
            .run_operation(&session_id, async move {
                crate::compaction::compact_session(
                    &*store,
                    &event_tx,
                    &*model,
                    &profile_alias,
                    &session_states,
                    workspace_id,
                    operation_session_id,
                    reason,
                )
                .await
            })
            .await
    }
}
