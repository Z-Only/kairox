//! Skills sub-trait — all methods provide default (no-op / error) implementations.

use crate::facade::{
    ActivateSkillRequest, ActiveSkillView, DeactivateSkillRequest, InstallGithubSkillRequest,
    InstallRemoteSkillRequest, RemoteSkillSearchResult, SkillCatalogEntry, SkillCatalogQuery,
    SkillDetail, SkillSettingsDetail, SkillSettingsView, SkillSourceView, SkillView,
};
use crate::SessionId;
use async_trait::async_trait;

#[async_trait]
pub trait SkillsFacade: Send + Sync {
    async fn list_skills(&self) -> crate::Result<Vec<SkillView>> {
        Ok(Vec::new())
    }

    async fn get_skill(&self, skill_id: String) -> crate::Result<Option<SkillDetail>> {
        let _ = skill_id;
        Ok(None)
    }

    async fn activate_skill(
        &self,
        request: ActivateSkillRequest,
    ) -> crate::Result<ActiveSkillView> {
        let _ = request;
        Err(crate::CoreError::InvalidState(
            "skill activation not supported".into(),
        ))
    }

    async fn deactivate_skill(&self, request: DeactivateSkillRequest) -> crate::Result<()> {
        let _ = request;
        Ok(())
    }

    async fn list_active_skills(
        &self,
        session_id: SessionId,
    ) -> crate::Result<Vec<ActiveSkillView>> {
        let _ = session_id;
        Ok(Vec::new())
    }

    async fn list_skill_settings(&self) -> crate::Result<Vec<SkillSettingsView>> {
        Ok(Vec::new())
    }

    async fn get_skill_settings_detail(
        &self,
        skill_id: String,
    ) -> crate::Result<Option<SkillSettingsDetail>> {
        let _ = skill_id;
        Ok(None)
    }

    async fn set_skill_enabled(&self, skill_id: String, enabled: bool) -> crate::Result<()> {
        let _ = (skill_id, enabled);
        Err(crate::CoreError::InvalidState(
            "Skill settings enablement not supported".into(),
        ))
    }

    async fn delete_skill_settings(&self, skill_id: String) -> crate::Result<()> {
        let _ = skill_id;
        Err(crate::CoreError::InvalidState(
            "Skill deletion not supported".into(),
        ))
    }

    async fn search_remote_skills(
        &self,
        query: String,
    ) -> crate::Result<Vec<RemoteSkillSearchResult>> {
        let _ = query;
        Ok(Vec::new())
    }

    async fn install_remote_skill(
        &self,
        request: InstallRemoteSkillRequest,
    ) -> crate::Result<SkillSettingsView> {
        let _ = request;
        Err(crate::CoreError::InvalidState(
            "Skill install not supported".into(),
        ))
    }

    async fn install_github_skill(
        &self,
        request: InstallGithubSkillRequest,
    ) -> crate::Result<SkillSettingsView> {
        let _ = request;
        Err(crate::CoreError::InvalidState(
            "GitHub Skill install not supported".into(),
        ))
    }

    async fn update_skill(&self, skill_id: String) -> crate::Result<SkillSettingsView> {
        let _ = skill_id;
        Err(crate::CoreError::InvalidState(
            "Skill update not supported".into(),
        ))
    }

    async fn list_skill_catalog(
        &self,
        _query: SkillCatalogQuery,
    ) -> crate::Result<Vec<SkillCatalogEntry>> {
        Ok(Vec::new())
    }

    async fn list_skill_sources(&self) -> crate::Result<Vec<SkillSourceView>> {
        Ok(Vec::new())
    }

    async fn add_skill_source(&self, _config: SkillSourceView) -> crate::Result<()> {
        Err(crate::CoreError::InvalidState(
            "skill sources not configured".into(),
        ))
    }

    async fn remove_skill_source(&self, _id: String) -> crate::Result<()> {
        Err(crate::CoreError::InvalidState(
            "skill sources not configured".into(),
        ))
    }

    async fn set_skill_source_enabled(&self, _id: String, _enabled: bool) -> crate::Result<()> {
        Err(crate::CoreError::InvalidState(
            "skill sources not configured".into(),
        ))
    }

    async fn refresh_skill_catalog(&self) -> crate::Result<()> {
        Ok(())
    }
}
