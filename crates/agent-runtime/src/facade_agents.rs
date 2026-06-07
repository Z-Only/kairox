use crate::facade_runtime::LocalRuntime;
use agent_core::facade::{AgentSettingsInput, AgentSettingsScope, AgentSettingsView, AgentsFacade};
use agent_store::EventStore;
use async_trait::async_trait;
use std::path::PathBuf;

fn normalize_project_root(project_root: Option<&str>) -> Option<PathBuf> {
    project_root
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from)
}

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    fn agent_settings_roots_for_project(
        &self,
        project_root: Option<&str>,
    ) -> crate::agent_settings::AgentSettingsRoots {
        let project_root = normalize_project_root(project_root);
        crate::agent_settings::roots_for_project(
            &self.agent_settings_roots(),
            project_root.as_deref(),
        )
    }

    pub async fn list_agent_settings_for_project(
        &self,
        project_root: Option<String>,
    ) -> agent_core::Result<Vec<AgentSettingsView>> {
        crate::agent_settings::list_agent_settings(
            self.agent_settings_roots_for_project(project_root.as_deref()),
        )
        .await
    }

    pub async fn upsert_agent_settings_for_project(
        &self,
        input: AgentSettingsInput,
        project_root: Option<String>,
    ) -> agent_core::Result<AgentSettingsView> {
        crate::agent_settings::upsert_agent_settings(
            self.agent_settings_roots_for_project(project_root.as_deref()),
            input,
        )
        .await
    }

    pub async fn delete_agent_settings_for_project(
        &self,
        agent_id: String,
        project_root: Option<String>,
    ) -> agent_core::Result<()> {
        crate::agent_settings::delete_agent_settings(
            self.agent_settings_roots_for_project(project_root.as_deref()),
            &agent_id,
        )
        .await
    }

    pub async fn copy_agent_settings_for_project(
        &self,
        agent_id: String,
        scope: AgentSettingsScope,
        project_root: Option<String>,
    ) -> agent_core::Result<AgentSettingsView> {
        crate::agent_settings::copy_agent_settings(
            self.agent_settings_roots_for_project(project_root.as_deref()),
            &agent_id,
            scope,
        )
        .await
    }

    pub async fn open_agents_dir_for_project(
        &self,
        project_root: Option<String>,
    ) -> agent_core::Result<Option<String>> {
        if normalize_project_root(project_root.as_deref()).is_none() {
            return self.open_user_agents_dir().await;
        }

        Ok(self
            .agent_settings_roots_for_project(project_root.as_deref())
            .workspace_root
            .map(|path| path.display().to_string()))
    }

    pub(crate) async fn open_user_agents_dir(&self) -> agent_core::Result<Option<String>> {
        Ok(self
            .agent_settings_roots()
            .user_root
            .map(|path| path.display().to_string()))
    }
}

#[async_trait]
impl<S, M> AgentsFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn list_agent_settings(&self) -> agent_core::Result<Vec<AgentSettingsView>> {
        LocalRuntime::list_agent_settings_for_project(self, None).await
    }

    async fn upsert_agent_settings(
        &self,
        input: AgentSettingsInput,
    ) -> agent_core::Result<AgentSettingsView> {
        LocalRuntime::upsert_agent_settings_for_project(self, input, None).await
    }

    async fn delete_agent_settings(&self, agent_id: String) -> agent_core::Result<()> {
        LocalRuntime::delete_agent_settings_for_project(self, agent_id, None).await
    }

    async fn copy_agent_settings(
        &self,
        agent_id: String,
        scope: AgentSettingsScope,
    ) -> agent_core::Result<AgentSettingsView> {
        LocalRuntime::copy_agent_settings_for_project(self, agent_id, scope, None).await
    }

    async fn open_agents_dir(&self) -> agent_core::Result<Option<String>> {
        LocalRuntime::open_user_agents_dir(self).await
    }
}

#[cfg(test)]
#[path = "facade_agents_tests.rs"]
mod facade_agents_tests;
