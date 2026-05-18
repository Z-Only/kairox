//! Agents sub-trait — custom agent settings and effective definitions.

use crate::facade::{AgentSettingsInput, AgentSettingsScope, AgentSettingsView};
use async_trait::async_trait;

#[async_trait]
pub trait AgentsFacade: Send + Sync {
    async fn list_agent_settings(&self) -> crate::Result<Vec<AgentSettingsView>> {
        Ok(Vec::new())
    }

    async fn upsert_agent_settings(
        &self,
        _input: AgentSettingsInput,
    ) -> crate::Result<AgentSettingsView> {
        Err(crate::CoreError::InvalidState(
            "agent settings not supported".into(),
        ))
    }

    async fn delete_agent_settings(&self, _agent_id: String) -> crate::Result<()> {
        Err(crate::CoreError::InvalidState(
            "agent deletion not supported".into(),
        ))
    }

    async fn copy_agent_settings(
        &self,
        _agent_id: String,
        _scope: AgentSettingsScope,
    ) -> crate::Result<AgentSettingsView> {
        Err(crate::CoreError::InvalidState(
            "agent copy not supported".into(),
        ))
    }

    async fn open_agents_dir(&self) -> crate::Result<Option<String>> {
        Ok(None)
    }
}
