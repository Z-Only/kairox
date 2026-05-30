use agent_core::{ProjectSessionVisibility, SessionId};
use agent_store::EventStore;

use crate::facade_runtime::LocalRuntime;

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub async fn mark_session_visible(
        &self,
        session_id: &SessionId,
        first_message: String,
    ) -> agent_core::Result<()> {
        let repository = self.project_repository()?;
        let draft_hidden =
            crate::project::visibility_to_storage(ProjectSessionVisibility::DraftHidden);
        let binding = repository
            .get_session_binding(session_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let visibility = repository
            .get_session_visibility(session_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        if binding.is_none() || visibility.as_deref() != Some(draft_hidden) {
            return Err(agent_core::CoreError::InvalidState(
                "only draft_hidden project sessions can be marked visible".into(),
            ));
        }

        repository
            .set_session_visibility(
                session_id.as_str(),
                crate::project::visibility_to_storage(ProjectSessionVisibility::Visible),
            )
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let title = crate::session::temporary_title_from_first_message(&first_message);
        self.store
            .rename_session(session_id.as_str(), &title)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
    }
}

#[cfg(test)]
#[path = "facade_sessions_tests.rs"]
mod tests;
