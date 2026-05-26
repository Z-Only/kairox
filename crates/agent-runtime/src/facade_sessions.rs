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
mod tests {
    use super::*;
    use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
    use agent_models::FakeModelClient;
    use agent_store::SqliteEventStore;

    #[tokio::test]
    async fn send_message_records_user_and_assistant_events() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hello".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),

                permission_mode: None,
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hi".into(),
                attachments: vec![],
            })
            .await
            .unwrap();

        let projection = runtime.get_session_projection(session_id).await.unwrap();
        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].content, "hi");
        assert_eq!(projection.messages[1].content, "hello");
    }

    #[tokio::test]
    async fn open_workspace_persists_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();

        let workspaces = runtime.list_workspaces().await.unwrap();
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].workspace_id, workspace.workspace_id);
        assert_eq!(workspaces[0].path, "/tmp/project");
    }

    #[tokio::test]
    async fn start_session_persists_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),

                permission_mode: None,
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, session_id);
        assert_eq!(sessions[0].title, "Session using fake");
    }

    #[tokio::test]
    async fn rename_session_updates_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),

                permission_mode: None,
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        runtime
            .rename_session(&session_id, "My Custom Title".into())
            .await
            .unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert_eq!(sessions[0].title, "My Custom Title");
    }

    #[tokio::test]
    async fn soft_delete_hides_session() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),

                permission_mode: None,
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        runtime.soft_delete_session(&session_id).await.unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert!(sessions.is_empty());
    }
}
