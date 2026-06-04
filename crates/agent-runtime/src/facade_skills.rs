use crate::facade_runtime::LocalRuntime;
use crate::skills::{
    skill_document_to_detail, skill_metadata_to_active_view, skill_metadata_to_view,
};
use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, RemoteSkillSearchResult,
    SkillCatalogEntry as CoreSkillCatalogEntry, SkillCatalogQuery as CoreSkillCatalogQuery,
    SkillSettingsDetail, SkillSettingsView, SkillSourceView as CoreSkillSourceView, SkillsFacade,
};
use agent_core::{
    ActivateSkillRequest, ActiveSkillView, AgentId, DeactivateSkillRequest, DomainEvent,
    EventPayload, PrivacyClassification, SessionId,
};
use agent_mcp::catalog::skills::{SkillCatalogProvider, SkillCatalogQuery};
use agent_store::{EventStore, ProjectMetaRepository};
use async_trait::async_trait;

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) async fn list_skills(
        &self,
    ) -> agent_core::Result<Vec<agent_core::facade::SkillView>> {
        self.list_skills_with_roots(self.skill_settings_roots())
            .await
    }

    pub async fn list_skills_with_roots(
        &self,
        roots: crate::skill_settings::SkillSettingsRoots,
    ) -> agent_core::Result<Vec<agent_core::facade::SkillView>> {
        let Some(registry) = self.skill_registry_for_roots(roots).await? else {
            return Ok(Vec::new());
        };
        Ok(registry.list().iter().map(skill_metadata_to_view).collect())
    }

    pub(crate) async fn get_skill(
        &self,
        skill_id: String,
    ) -> agent_core::Result<Option<agent_core::facade::SkillDetail>> {
        self.get_skill_with_roots(self.skill_settings_roots(), skill_id)
            .await
    }

    pub async fn get_skill_with_roots(
        &self,
        roots: crate::skill_settings::SkillSettingsRoots,
        skill_id: String,
    ) -> agent_core::Result<Option<agent_core::facade::SkillDetail>> {
        let Some(registry) = self.skill_registry_for_roots(roots).await? else {
            return Ok(None);
        };
        let skill_id = agent_skills::SkillId::new(skill_id);
        if registry.get(&skill_id).is_none() {
            return Ok(None);
        }
        let document = registry
            .load_document(&skill_id)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(Some(skill_document_to_detail(document)))
    }

    pub(crate) async fn activate_skill(
        &self,
        request: ActivateSkillRequest,
    ) -> agent_core::Result<ActiveSkillView> {
        let roots = self
            .skill_settings_roots_for_session(&request.session_id)
            .await;
        self.activate_skill_with_roots(roots, request).await
    }

    pub async fn activate_skill_with_roots(
        &self,
        roots: crate::skill_settings::SkillSettingsRoots,
        request: ActivateSkillRequest,
    ) -> agent_core::Result<ActiveSkillView> {
        let registry = self.skill_registry_for_roots(roots).await?.ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill registry not configured".into())
        })?;
        let skill_id = agent_skills::SkillId::new(request.skill_id.clone());
        let metadata = registry.get(&skill_id).ok_or_else(|| {
            agent_core::CoreError::InvalidState(format!("skill not found: {}", request.skill_id))
        })?;
        let active_view = skill_metadata_to_active_view(&metadata);

        let activated = {
            let mut active_skills = self.active_skills.lock().await;
            let session_skills = active_skills
                .entry(request.session_id.to_string())
                .or_insert_with(Vec::new);
            if session_skills.iter().any(|id| id == &request.skill_id) {
                false
            } else {
                session_skills.push(request.skill_id.clone());
                true
            }
        };

        if activated {
            let event = DomainEvent::new(
                request.workspace_id,
                request.session_id,
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::SkillActivated {
                    skill_id: active_view.skill_id.clone(),
                    name: active_view.name.clone(),
                    source: active_view.source.clone(),
                    activation_mode: active_view.activation_mode.clone(),
                },
            );
            crate::event_emitter::append_and_broadcast(&*self.store, &self.event_tx, &event)
                .await?;
        }

        Ok(active_view)
    }

    pub(crate) async fn deactivate_skill(
        &self,
        request: DeactivateSkillRequest,
    ) -> agent_core::Result<()> {
        let roots = self
            .skill_settings_roots_for_session(&request.session_id)
            .await;
        self.deactivate_skill_with_roots(roots, request).await
    }

    pub async fn deactivate_skill_with_roots(
        &self,
        roots: crate::skill_settings::SkillSettingsRoots,
        request: DeactivateSkillRequest,
    ) -> agent_core::Result<()> {
        let registry = self.skill_registry_for_roots(roots).await?.ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill registry not configured".into())
        })?;
        let skill_id = agent_skills::SkillId::new(request.skill_id.clone());
        let metadata = registry.get(&skill_id).ok_or_else(|| {
            agent_core::CoreError::InvalidState(format!("skill not found: {}", request.skill_id))
        })?;
        let active_view = skill_metadata_to_active_view(&metadata);

        let removed = {
            let mut active_skills = self.active_skills.lock().await;
            let Some(session_skills) = active_skills.get_mut(&request.session_id.to_string())
            else {
                return Ok(());
            };
            let original_len = session_skills.len();
            session_skills.retain(|id| id != &request.skill_id);
            session_skills.len() != original_len
        };
        if !removed {
            return Ok(());
        }

        let event = DomainEvent::new(
            request.workspace_id,
            request.session_id,
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::SkillDeactivated {
                skill_id: active_view.skill_id,
                name: active_view.name,
                source: active_view.source,
            },
        );
        crate::event_emitter::append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    pub(crate) async fn list_active_skills(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<Vec<ActiveSkillView>> {
        let roots = self.skill_settings_roots_for_session(&session_id).await;
        self.list_active_skills_with_roots(roots, session_id).await
    }

    pub async fn list_active_skills_with_roots(
        &self,
        roots: crate::skill_settings::SkillSettingsRoots,
        session_id: SessionId,
    ) -> agent_core::Result<Vec<ActiveSkillView>> {
        let Some(registry) = self.skill_registry_for_roots(roots).await? else {
            return Ok(Vec::new());
        };
        let session_key = session_id.to_string();
        let skill_ids = {
            let active_skills = self.active_skills.lock().await;
            active_skills.get(&session_key).cloned()
        };
        let skill_ids = match skill_ids {
            Some(skill_ids) => skill_ids,
            None => {
                let events = self
                    .store
                    .load_session(&session_id)
                    .await
                    .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
                crate::skills::active_skill_ids_from_events(&events)
            }
        };

        let mut active_views = Vec::new();
        let mut retained_skill_ids = Vec::new();
        for skill_id in skill_ids {
            let skill_id_value = agent_skills::SkillId::new(skill_id.clone());
            if let Some(metadata) = registry.get(&skill_id_value) {
                active_views.push(skill_metadata_to_active_view(&metadata));
                retained_skill_ids.push(skill_id);
            }
        }

        let mut active_skills = self.active_skills.lock().await;
        active_skills.insert(session_key, retained_skill_ids);

        Ok(active_views)
    }

    pub async fn skill_settings_roots_for_session(
        &self,
        session_id: &SessionId,
    ) -> crate::skill_settings::SkillSettingsRoots {
        let roots = self.skill_settings_roots();
        let Some(repository) = self.store.sqlite_pool().map(ProjectMetaRepository::new) else {
            return roots;
        };
        match repository.get_session_binding(session_id.as_str()).await {
            Ok(Some(binding)) => crate::skills::skill_settings_roots_for_project_root(
                roots,
                std::path::Path::new(&binding.worktree_path),
            ),
            _ => roots,
        }
    }

    pub async fn skill_registry_for_roots(
        &self,
        roots: crate::skill_settings::SkillSettingsRoots,
    ) -> agent_core::Result<Option<std::sync::Arc<dyn agent_skills::SkillRegistry>>> {
        crate::skills::discover_skill_registry_for_settings_roots(
            roots,
            self.skill_registry.clone(),
        )
        .await
    }

    pub(crate) async fn list_skill_settings(&self) -> agent_core::Result<Vec<SkillSettingsView>> {
        crate::skill_settings::list_skill_settings(self.skill_settings_roots()).await
    }

    pub(crate) async fn get_skill_settings_detail(
        &self,
        skill_id: String,
    ) -> agent_core::Result<Option<SkillSettingsDetail>> {
        crate::skill_settings::get_skill_settings_detail(self.skill_settings_roots(), &skill_id)
            .await
    }

    pub(crate) async fn set_skill_enabled(
        &self,
        skill_id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        crate::skill_settings::set_skill_enabled(self.skill_settings_roots(), &skill_id, enabled)
            .await
    }

    pub(crate) async fn delete_skill_settings(&self, skill_id: String) -> agent_core::Result<()> {
        crate::skill_settings::delete_skill(self.skill_settings_roots(), &skill_id).await
    }

    pub(crate) async fn search_remote_skills(
        &self,
        query: String,
    ) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
        self.skill_package_manager.search(&query).await
    }

    pub(crate) async fn install_remote_skill(
        &self,
        request: InstallRemoteSkillRequest,
    ) -> agent_core::Result<SkillSettingsView> {
        self.install_remote_skill_with_roots(self.skill_settings_roots(), request)
            .await
    }

    pub async fn install_remote_skill_with_roots(
        &self,
        roots: crate::skill_settings::SkillSettingsRoots,
        request: InstallRemoteSkillRequest,
    ) -> agent_core::Result<SkillSettingsView> {
        crate::skill_settings::install_remote_skill(
            roots,
            self.skill_package_manager.as_ref(),
            request,
        )
        .await
    }

    pub(crate) async fn install_github_skill(
        &self,
        request: InstallGithubSkillRequest,
    ) -> agent_core::Result<SkillSettingsView> {
        self.install_github_skill_with_roots(self.skill_settings_roots(), request)
            .await
    }

    pub async fn install_github_skill_with_roots(
        &self,
        roots: crate::skill_settings::SkillSettingsRoots,
        request: InstallGithubSkillRequest,
    ) -> agent_core::Result<SkillSettingsView> {
        crate::skill_settings::install_github_skill(
            roots,
            self.skill_package_manager.as_ref(),
            request,
        )
        .await
    }

    pub(crate) async fn update_skill(
        &self,
        skill_id: String,
    ) -> agent_core::Result<SkillSettingsView> {
        self.update_skill_with_roots(self.skill_settings_roots(), skill_id)
            .await
    }

    pub async fn update_skill_with_roots(
        &self,
        roots: crate::skill_settings::SkillSettingsRoots,
        skill_id: String,
    ) -> agent_core::Result<SkillSettingsView> {
        crate::skill_settings::update_skill(roots, self.skill_package_manager.as_ref(), &skill_id)
            .await
    }

    pub(crate) async fn list_skill_catalog(
        &self,
        query: CoreSkillCatalogQuery,
    ) -> agent_core::Result<Vec<CoreSkillCatalogEntry>> {
        let catalog = self.ensure_skill_catalog().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill catalog not configured".into())
        })?;

        let inner_query = SkillCatalogQuery {
            keyword: query.keyword,
            sources: query.sources,
            limit: query.limit,
        };

        let entries = catalog
            .search(&inner_query)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("skill catalog: {e}")))?;

        Ok(entries
            .into_iter()
            .map(|e| CoreSkillCatalogEntry {
                catalog_id: e.catalog_id,
                name: e.name,
                description: e.description,
                source: e.source,
                source_url: e.source_url,
                install_count: e.install_count,
                github_stars: e.github_stars,
                security_score: e.security_score,
                rating: e.rating,
                package: e.package,
                package_url: e.package_url,
            })
            .collect())
    }

    pub(crate) async fn list_skill_sources(&self) -> agent_core::Result<Vec<CoreSkillSourceView>> {
        let sources = match &self.skill_sources_toml {
            Some(toml) => toml.merge_with_defaults(&toml.read()),
            None => crate::skill_sources_toml::default_skill_sources(),
        };
        Ok(sources)
    }

    pub(crate) async fn add_skill_source(
        &self,
        config: CoreSkillSourceView,
    ) -> agent_core::Result<()> {
        let toml = self.skill_sources_toml.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill sources not configured".into())
        })?;
        let mut sources = toml.read();
        sources.retain(|s| s.id != config.id);
        sources.push(config);
        toml.write(&sources)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("write: {e}")))?;
        self.rebuild_skill_aggregate()?;
        Ok(())
    }

    pub(crate) async fn remove_skill_source(&self, id: String) -> agent_core::Result<()> {
        let toml = self.skill_sources_toml.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill sources not configured".into())
        })?;
        let mut sources = toml.read();
        sources.retain(|s| s.id != id);
        toml.write(&sources)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("write: {e}")))?;
        self.rebuild_skill_aggregate()?;
        Ok(())
    }

    pub(crate) async fn set_skill_source_enabled(
        &self,
        id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        let toml = self.skill_sources_toml.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill sources not configured".into())
        })?;
        let mut sources = toml.read();
        if let Some(s) = sources.iter_mut().find(|s| s.id == id) {
            s.enabled = enabled;
        }
        toml.write(&sources)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("write: {e}")))?;
        self.rebuild_skill_aggregate()?;
        Ok(())
    }

    pub(crate) async fn open_skills_dir(&self) -> agent_core::Result<Option<String>> {
        let dir = std::env::var("HOME").ok().map(|h| {
            std::path::PathBuf::from(h)
                .join(".config")
                .join("kairox")
                .join("skills")
        });
        Ok(dir.map(|d| d.display().to_string()))
    }

    pub(crate) async fn refresh_skill_catalog(&self) -> agent_core::Result<()> {
        let catalog = self.ensure_skill_catalog().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill catalog not configured".into())
        })?;
        catalog.refresh().await.map_err(|e| {
            agent_core::CoreError::InvalidState(format!("skill catalog refresh: {e}"))
        })?;
        Ok(())
    }
}

#[async_trait]
impl<S, M> SkillsFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn list_skills(&self) -> agent_core::Result<Vec<agent_core::facade::SkillView>> {
        LocalRuntime::list_skills(self).await
    }

    async fn get_skill(
        &self,
        skill_id: String,
    ) -> agent_core::Result<Option<agent_core::facade::SkillDetail>> {
        LocalRuntime::get_skill(self, skill_id).await
    }

    async fn activate_skill(
        &self,
        request: ActivateSkillRequest,
    ) -> agent_core::Result<ActiveSkillView> {
        LocalRuntime::activate_skill(self, request).await
    }

    async fn deactivate_skill(&self, request: DeactivateSkillRequest) -> agent_core::Result<()> {
        LocalRuntime::deactivate_skill(self, request).await
    }

    async fn list_active_skills(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<Vec<ActiveSkillView>> {
        LocalRuntime::list_active_skills(self, session_id).await
    }

    async fn list_skill_settings(&self) -> agent_core::Result<Vec<SkillSettingsView>> {
        LocalRuntime::list_skill_settings(self).await
    }

    async fn get_skill_settings_detail(
        &self,
        skill_id: String,
    ) -> agent_core::Result<Option<SkillSettingsDetail>> {
        LocalRuntime::get_skill_settings_detail(self, skill_id).await
    }

    async fn set_skill_enabled(&self, skill_id: String, enabled: bool) -> agent_core::Result<()> {
        LocalRuntime::set_skill_enabled(self, skill_id, enabled).await
    }

    async fn delete_skill_settings(&self, skill_id: String) -> agent_core::Result<()> {
        LocalRuntime::delete_skill_settings(self, skill_id).await
    }

    async fn search_remote_skills(
        &self,
        query: String,
    ) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
        LocalRuntime::search_remote_skills(self, query).await
    }

    async fn install_remote_skill(
        &self,
        request: InstallRemoteSkillRequest,
    ) -> agent_core::Result<SkillSettingsView> {
        LocalRuntime::install_remote_skill(self, request).await
    }

    async fn install_github_skill(
        &self,
        request: InstallGithubSkillRequest,
    ) -> agent_core::Result<SkillSettingsView> {
        LocalRuntime::install_github_skill(self, request).await
    }

    async fn update_skill(&self, skill_id: String) -> agent_core::Result<SkillSettingsView> {
        LocalRuntime::update_skill(self, skill_id).await
    }

    async fn list_skill_catalog(
        &self,
        query: CoreSkillCatalogQuery,
    ) -> agent_core::Result<Vec<CoreSkillCatalogEntry>> {
        LocalRuntime::list_skill_catalog(self, query).await
    }

    async fn list_skill_sources(&self) -> agent_core::Result<Vec<CoreSkillSourceView>> {
        LocalRuntime::list_skill_sources(self).await
    }

    async fn add_skill_source(&self, config: CoreSkillSourceView) -> agent_core::Result<()> {
        LocalRuntime::add_skill_source(self, config).await
    }

    async fn remove_skill_source(&self, id: String) -> agent_core::Result<()> {
        LocalRuntime::remove_skill_source(self, id).await
    }

    async fn set_skill_source_enabled(&self, id: String, enabled: bool) -> agent_core::Result<()> {
        LocalRuntime::set_skill_source_enabled(self, id, enabled).await
    }

    async fn refresh_skill_catalog(&self) -> agent_core::Result<()> {
        LocalRuntime::refresh_skill_catalog(self).await
    }

    async fn open_skills_dir(&self) -> agent_core::Result<Option<String>> {
        LocalRuntime::open_skills_dir(self).await
    }
}
