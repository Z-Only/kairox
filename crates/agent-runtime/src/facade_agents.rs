use crate::facade_runtime::LocalRuntime;
use agent_core::facade::{AgentSettingsInput, AgentSettingsScope, AgentSettingsView, AgentsFacade};
use agent_store::EventStore;
use async_trait::async_trait;

#[async_trait]
impl<S, M> AgentsFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn list_agent_settings(&self) -> agent_core::Result<Vec<AgentSettingsView>> {
        crate::agent_settings::list_agent_settings(self.agent_settings_roots()).await
    }

    async fn upsert_agent_settings(
        &self,
        input: AgentSettingsInput,
    ) -> agent_core::Result<AgentSettingsView> {
        crate::agent_settings::upsert_agent_settings(self.agent_settings_roots(), input).await
    }

    async fn delete_agent_settings(&self, agent_id: String) -> agent_core::Result<()> {
        crate::agent_settings::delete_agent_settings(self.agent_settings_roots(), &agent_id).await
    }

    async fn copy_agent_settings(
        &self,
        agent_id: String,
        scope: AgentSettingsScope,
    ) -> agent_core::Result<AgentSettingsView> {
        crate::agent_settings::copy_agent_settings(self.agent_settings_roots(), &agent_id, scope)
            .await
    }

    async fn open_agents_dir(&self) -> agent_core::Result<Option<String>> {
        Ok(self
            .agent_settings_roots()
            .user_root
            .map(|path| path.display().to_string()))
    }
}
