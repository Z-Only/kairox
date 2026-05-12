use crate::facade_runtime::LocalRuntime;
use agent_core::facade::{
    McpFacade, McpServerSettingsInput, McpServerSettingsView, ProfileSettingsInput,
    ProfileSettingsView,
};
use agent_core::{
    AddCatalogSourceRequest, AgentId, CatalogQuery as CoreCatalogQuery, CatalogSourceView,
    DomainEvent, EventPayload, InstallOutcomeView as CoreInstallOutcomeView,
    InstallRequest as CoreInstallRequest, InstalledEntry as CoreInstalledEntry,
    PrivacyClassification, ServerEntry as CoreServerEntry, SessionId, WorkspaceId,
};
use agent_mcp::catalog::{
    CatalogProvider, CatalogQuery, InstallRequest as McpInstallRequest, ServerEntry, TrustLevel,
};
use agent_mcp::installer::InstallOutcomeView;
use agent_mcp::types::{McpServerDef, McpTransportDef};
use agent_mcp::InstallSpec;
use agent_store::EventStore;
use async_trait::async_trait;
use std::sync::Arc;

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) async fn list_mcp_server_settings(
        &self,
    ) -> agent_core::Result<Vec<McpServerSettingsView>> {
        let config_path =
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?;
        crate::mcp_settings::list_mcp_server_settings(
            &self.config,
            config_path.as_deref(),
            self.mcp_manager.clone(),
        )
        .await
    }

    pub(crate) async fn upsert_mcp_server_settings(
        &self,
        input: McpServerSettingsInput,
    ) -> agent_core::Result<McpServerSettingsView> {
        let config_path =
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?
                .ok_or_else(|| {
                    agent_core::CoreError::InvalidState(
                        "marketplace install dir not configured; cannot write MCP settings".into(),
                    )
                })?;
        crate::mcp_settings::upsert_mcp_server_settings(&config_path, input).await
    }

    pub(crate) async fn delete_mcp_server_settings(
        &self,
        server_id: String,
    ) -> agent_core::Result<()> {
        let config_path =
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?
                .ok_or_else(|| {
                    agent_core::CoreError::InvalidState(
                        "marketplace install dir not configured; cannot write MCP settings".into(),
                    )
                })?;
        crate::mcp_settings::delete_mcp_server_settings(
            &config_path,
            self.mcp_manager.clone(),
            &server_id,
        )
        .await
    }

    pub(crate) async fn set_mcp_server_enabled(
        &self,
        server_id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        let config_path =
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?
                .ok_or_else(|| {
                    agent_core::CoreError::InvalidState(
                        "marketplace install dir not configured; cannot write MCP settings".into(),
                    )
                })?;
        crate::mcp_settings::set_mcp_server_enabled(
            &config_path,
            self.mcp_manager.clone(),
            &server_id,
            enabled,
        )
        .await
    }

    pub(crate) async fn open_mcp_config_file(&self) -> agent_core::Result<Option<String>> {
        Ok(
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?
                .map(|path| path.display().to_string()),
        )
    }

    pub(crate) async fn list_profile_settings(
        &self,
        source_filter: Option<String>,
    ) -> agent_core::Result<Vec<ProfileSettingsView>> {
        let profiles_toml_path = crate::profile_settings::writable_profiles_config_path(
            self.marketplace_dir.as_deref(),
        )?;
        let user_config_path = std::env::var("HOME").ok().map(|h| {
            std::path::PathBuf::from(h)
                .join(".kairox")
                .join("config.toml")
        });
        let project_config_path = std::env::current_dir()
            .ok()
            .map(|d| d.join(".kairox").join("config.toml"));
        crate::profile_settings::list_profile_settings(
            &self.config,
            profiles_toml_path.as_deref(),
            user_config_path.as_deref(),
            project_config_path.as_deref(),
            source_filter.as_deref(),
        )
        .await
    }

    pub(crate) async fn upsert_profile_settings(
        &self,
        input: ProfileSettingsInput,
    ) -> agent_core::Result<ProfileSettingsView> {
        let config_path = crate::profile_settings::writable_profiles_config_path(
            self.marketplace_dir.as_deref(),
        )?
        .ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "config dir not configured; cannot write profile settings".into(),
            )
        })?;
        crate::profile_settings::upsert_profile_settings_in_file(&config_path, &input).await
    }

    pub(crate) async fn set_profile_enabled(
        &self,
        alias: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        let config_path = crate::profile_settings::writable_profiles_config_path(
            self.marketplace_dir.as_deref(),
        )?
        .ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "config dir not configured; cannot write profile settings".into(),
            )
        })?;
        crate::profile_settings::set_profile_enabled_in_file(
            &config_path,
            &alias,
            enabled,
            &self.config,
        )
        .await
    }

    pub(crate) async fn delete_profile_settings(&self, alias: String) -> agent_core::Result<()> {
        let config_path = crate::profile_settings::writable_profiles_config_path(
            self.marketplace_dir.as_deref(),
        )?
        .ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "config dir not configured; cannot write profile settings".into(),
            )
        })?;
        crate::profile_settings::delete_profile_in_file(&config_path, &alias).await
    }

    pub(crate) async fn move_profile_in_order(
        &self,
        alias: String,
        direction: i32,
    ) -> agent_core::Result<()> {
        let config_path = crate::profile_settings::writable_profiles_config_path(
            self.marketplace_dir.as_deref(),
        )?
        .ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "config dir not configured; cannot reorder profiles".into(),
            )
        })?;
        crate::profile_settings::move_profile_in_order(&config_path, &alias, direction).await
    }

    pub(crate) async fn open_config_dir(&self) -> agent_core::Result<Option<String>> {
        Ok(self
            .marketplace_dir
            .as_ref()
            .map(|p| p.display().to_string()))
    }

    // -----------------------------------------------------------------------
    // Marketplace catalog
    // -----------------------------------------------------------------------
    pub(crate) async fn list_catalog(
        &self,
        query: CoreCatalogQuery,
    ) -> agent_core::Result<Vec<CoreServerEntry>> {
        let inner_query = map_query(query);
        let entries = match self.catalog.as_ref() {
            Some(catalog) => catalog
                .list(&inner_query)
                .await
                .map_err(|e| agent_core::CoreError::InvalidState(format!("catalog list: {e}")))?,
            None => {
                let builtin = builtin_only_provider()?;
                builtin.list(&inner_query).await.map_err(|e| {
                    agent_core::CoreError::InvalidState(format!("catalog list: {e}"))
                })?
            }
        };
        Ok(entries.into_iter().map(map_entry_to_core).collect())
    }

    pub(crate) async fn get_catalog_entry(
        &self,
        id: String,
        _source: Option<String>,
    ) -> agent_core::Result<Option<CoreServerEntry>> {
        let entry =
            match self.catalog.as_ref() {
                Some(catalog) => catalog.get(&id).await.map_err(|e| {
                    agent_core::CoreError::InvalidState(format!("catalog get: {e}"))
                })?,
                None => {
                    let builtin = builtin_only_provider()?;
                    builtin.get(&id).await.map_err(|e| {
                        agent_core::CoreError::InvalidState(format!("catalog get: {e}"))
                    })?
                }
            };
        Ok(entry.map(map_entry_to_core))
    }

    pub(crate) async fn refresh_catalog(&self, _source: Option<String>) -> agent_core::Result<()> {
        let Some(catalog) = self.catalog.as_ref() else {
            return Ok(());
        };

        if self.marketplace_dir.is_some() {
            self.rebuild_aggregate_from_disk().await?;
        }

        catalog
            .refresh()
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("catalog refresh: {e}")))?;
        let entry_count = catalog
            .list(&CatalogQuery::default())
            .await
            .map(|v| v.len())
            .unwrap_or(0);
        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogRefreshed {
                source: "aggregate".into(),
                entry_count,
            },
        );
        Ok(())
    }

    pub(crate) async fn install_catalog_entry(
        &self,
        request: CoreInstallRequest,
    ) -> agent_core::Result<CoreInstallOutcomeView> {
        let catalog = self.catalog.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "marketplace install dir not configured; cannot install".into(),
            )
        })?;
        let installer = self.installer.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "marketplace install dir not configured; cannot install".into(),
            )
        })?;

        let inner_req = map_install_request(request);
        let entry = catalog
            .get(&inner_req.catalog_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("catalog: {e}")))?
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!(
                    "entry not found: {}",
                    inner_req.catalog_id
                ))
            })?;

        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogEntryInstalling {
                catalog_id: inner_req.catalog_id.clone(),
                source: inner_req.source.clone(),
            },
        );

        let outcome = installer
            .install(&entry, &inner_req)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("installer: {e}")))?;

        match &outcome {
            InstallOutcomeView::RuntimeMissing { missing } => {
                emit_marketplace_event(
                    &self.event_tx,
                    EventPayload::CatalogRuntimeMissing {
                        catalog_id: inner_req.catalog_id.clone(),
                        missing: missing.iter().map(|r| r.kind.as_str().into()).collect(),
                    },
                );
            }
            InstallOutcomeView::Installed { server_id, started } => {
                if let Some(manager) = &self.mcp_manager {
                    let def = build_server_def(&entry, &inner_req);
                    let mut mgr = manager.lock().await;
                    if !mgr.is_registered(server_id) {
                        if let Err(e) = mgr.register_dynamic(def) {
                            tracing::warn!(
                                "marketplace install: register_dynamic({server_id}) failed: {e}"
                            );
                        }
                    }
                    if *started {
                        if let Err(e) = mgr.ensure_server(server_id).await {
                            tracing::warn!(
                                "marketplace install: ensure_server({server_id}) failed: {e}"
                            );
                        }
                    }
                }
                emit_marketplace_event(
                    &self.event_tx,
                    EventPayload::CatalogEntryInstalled {
                        catalog_id: inner_req.catalog_id.clone(),
                        source: inner_req.source.clone(),
                        server_id: server_id.clone(),
                    },
                );
            }
            _ => {}
        }
        Ok(map_outcome_to_core(outcome))
    }

    pub(crate) async fn uninstall_catalog_entry(
        &self,
        server_id: String,
    ) -> agent_core::Result<()> {
        let installer = self.installer.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "marketplace install dir not configured; cannot uninstall".into(),
            )
        })?;
        installer
            .uninstall(&server_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("installer: {e}")))?;
        if let Some(manager) = &self.mcp_manager {
            if let Err(e) = manager.lock().await.unregister_dynamic(&server_id).await {
                tracing::warn!(
                    "marketplace uninstall: unregister_dynamic({server_id}) failed: {e}"
                );
            }
        }
        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogEntryUninstalled {
                server_id: server_id.clone(),
            },
        );
        Ok(())
    }

    pub(crate) async fn list_installed_entries(
        &self,
    ) -> agent_core::Result<Vec<CoreInstalledEntry>> {
        let Some(installer) = self.installer.as_ref() else {
            return Ok(Vec::new());
        };
        let ids = installer
            .list_installed_ids()
            .map_err(|e| agent_core::CoreError::InvalidState(format!("installer: {e}")))?;

        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let entry = if let Some(c) = &self.catalog {
                c.get(&id).await.ok().flatten()
            } else {
                None
            };
            let running = if let Some(manager) = &self.mcp_manager {
                manager.lock().await.is_running(&id).unwrap_or(false)
            } else {
                false
            };
            let display_name = entry
                .as_ref()
                .map(|e| e.display_name.clone())
                .unwrap_or_else(|| id.clone());
            out.push(CoreInstalledEntry {
                server_id: id,
                catalog_id: entry.as_ref().map(|e| e.id.clone()),
                source: entry.as_ref().map(|e| e.source.clone()),
                display_name,
                installed_at: chrono::Utc::now().to_rfc3339(),
                running,
            });
        }
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // Phase 2: catalog source mutations
    // -----------------------------------------------------------------------

    pub(crate) async fn list_catalog_sources(&self) -> agent_core::Result<Vec<CatalogSourceView>> {
        let builtin_view = builtin_source_view();

        let user_sources = match self.marketplace_dir.as_ref() {
            Some(dir) => {
                let mt = crate::marketplace_toml::MarketplaceToml::new(dir);
                mt.read_sources().map_err(|e| {
                    agent_core::CoreError::InvalidState(format!("marketplace toml: {e}"))
                })?
            }
            None => Vec::new(),
        };
        let merged = agent_config::merge_with_defaults(user_sources);

        let mut out = Vec::with_capacity(merged.len() + 1);
        out.push(builtin_view);
        for s in merged {
            out.push(catalog_source_to_view(s));
        }
        Ok(out)
    }

    pub(crate) async fn add_catalog_source(
        &self,
        request: AddCatalogSourceRequest,
    ) -> agent_core::Result<()> {
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let cfg = request_to_source_config(request)?;
        let id = cfg.id.clone();
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.add_source(cfg)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("marketplace toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await?;
        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogSourceAdded { source: id },
        );
        Ok(())
    }

    pub(crate) async fn remove_catalog_source(&self, id: String) -> agent_core::Result<()> {
        if id == "builtin" {
            return Ok(());
        }
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.remove_source(&id)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("marketplace toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await
    }

    pub(crate) async fn set_catalog_source_enabled(
        &self,
        id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        if id == "builtin" {
            return Ok(());
        }
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.set_enabled(&id, enabled)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("marketplace toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await
    }

    /// Phase 2: rebuild the aggregate's remote provider list from
    /// `<marketplace_dir>/mcp_servers.toml`, calling
    /// [`AggregateCatalogProvider::reload`]. The builtin provider is
    /// always re-added at priority 0.
    pub(crate) async fn rebuild_aggregate_from_disk(&self) -> agent_core::Result<()> {
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let aggregate = self.aggregate_handle.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let http = self.catalog_http.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let cache = self.catalog_cache.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;

        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        let user_sources = mt
            .read_sources()
            .map_err(|e| agent_core::CoreError::InvalidState(format!("marketplace toml: {e}")))?;
        let sources = agent_config::merge_with_defaults(user_sources);

        let mut providers: Vec<(u32, Arc<dyn CatalogProvider>)> = Vec::new();
        let builtin = Arc::new(
            agent_mcp::catalog::BuiltinCatalogProvider::new().map_err(|e| {
                agent_core::CoreError::InvalidState(format!("builtin catalog: {e}"))
            })?,
        );
        providers.push((0, builtin));
        for s in sources.iter().filter(|s| s.enabled) {
            let cfg = agent_mcp::RemoteSourceConfig {
                id: s.id.clone(),
                display_name: s.display_name.clone(),
                kind: match s.kind {
                    agent_config::CatalogSourceKind::McpRegistry => {
                        agent_mcp::RemoteSourceKind::McpRegistry
                    }
                },
                url: s.url.clone(),
                api_key_env: s.api_key_env.clone(),
                priority: s.priority,
                default_trust: crate::facade_runtime::parse_trust_str(&s.default_trust),
                enabled: true,
                cache_ttl_seconds: s.cache_ttl_seconds,
            };
            providers.push((
                s.priority,
                agent_mcp::build_remote_catalog_provider(cfg, http.clone(), cache.clone()),
            ));
        }
        aggregate.reload(providers);
        Ok(())
    }
}

// ── McpFacade trait impl (delegates to inherent methods above) ──────────

#[async_trait]
impl<S, M> McpFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn list_mcp_server_settings(&self) -> agent_core::Result<Vec<McpServerSettingsView>> {
        LocalRuntime::list_mcp_server_settings(self).await
    }

    async fn upsert_mcp_server_settings(
        &self,
        input: McpServerSettingsInput,
    ) -> agent_core::Result<McpServerSettingsView> {
        LocalRuntime::upsert_mcp_server_settings(self, input).await
    }

    async fn delete_mcp_server_settings(&self, server_id: String) -> agent_core::Result<()> {
        LocalRuntime::delete_mcp_server_settings(self, server_id).await
    }

    async fn set_mcp_server_enabled(
        &self,
        server_id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        LocalRuntime::set_mcp_server_enabled(self, server_id, enabled).await
    }

    async fn open_mcp_config_file(&self) -> agent_core::Result<Option<String>> {
        LocalRuntime::open_mcp_config_file(self).await
    }

    async fn list_profile_settings(
        &self,
        source_filter: Option<String>,
    ) -> agent_core::Result<Vec<ProfileSettingsView>> {
        LocalRuntime::list_profile_settings(self, source_filter).await
    }

    async fn upsert_profile_settings(
        &self,
        input: ProfileSettingsInput,
    ) -> agent_core::Result<ProfileSettingsView> {
        LocalRuntime::upsert_profile_settings(self, input).await
    }

    async fn set_profile_enabled(&self, alias: String, enabled: bool) -> agent_core::Result<()> {
        LocalRuntime::set_profile_enabled(self, alias, enabled).await
    }

    async fn delete_profile_settings(&self, alias: String) -> agent_core::Result<()> {
        LocalRuntime::delete_profile_settings(self, alias).await
    }

    async fn move_profile_in_order(&self, alias: String, direction: i32) -> agent_core::Result<()> {
        LocalRuntime::move_profile_in_order(self, alias, direction).await
    }

    async fn open_config_dir(&self) -> agent_core::Result<Option<String>> {
        LocalRuntime::open_config_dir(self).await
    }

    async fn list_catalog(
        &self,
        query: CoreCatalogQuery,
    ) -> agent_core::Result<Vec<CoreServerEntry>> {
        LocalRuntime::list_catalog(self, query).await
    }

    async fn get_catalog_entry(
        &self,
        id: String,
        source: Option<String>,
    ) -> agent_core::Result<Option<CoreServerEntry>> {
        LocalRuntime::get_catalog_entry(self, id, source).await
    }

    async fn refresh_catalog(&self, source: Option<String>) -> agent_core::Result<()> {
        LocalRuntime::refresh_catalog(self, source).await
    }

    async fn install_catalog_entry(
        &self,
        request: CoreInstallRequest,
    ) -> agent_core::Result<CoreInstallOutcomeView> {
        LocalRuntime::install_catalog_entry(self, request).await
    }

    async fn uninstall_catalog_entry(&self, server_id: String) -> agent_core::Result<()> {
        LocalRuntime::uninstall_catalog_entry(self, server_id).await
    }

    async fn list_installed_entries(&self) -> agent_core::Result<Vec<CoreInstalledEntry>> {
        LocalRuntime::list_installed_entries(self).await
    }

    async fn list_catalog_sources(&self) -> agent_core::Result<Vec<CatalogSourceView>> {
        LocalRuntime::list_catalog_sources(self).await
    }

    async fn add_catalog_source(&self, request: AddCatalogSourceRequest) -> agent_core::Result<()> {
        LocalRuntime::add_catalog_source(self, request).await
    }

    async fn remove_catalog_source(&self, id: String) -> agent_core::Result<()> {
        LocalRuntime::remove_catalog_source(self, id).await
    }

    async fn set_catalog_source_enabled(
        &self,
        id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        LocalRuntime::set_catalog_source_enabled(self, id, enabled).await
    }
}

// ---------------------------------------------------------------------------
// Marketplace mapping helpers
// ---------------------------------------------------------------------------

fn map_query(q: CoreCatalogQuery) -> CatalogQuery {
    CatalogQuery {
        keyword: q.keyword,
        category: q.category,
        trust_min: q.trust_min.as_deref().and_then(parse_trust),
        source: q.source,
        limit: q.limit,
    }
}

fn parse_trust(s: &str) -> Option<TrustLevel> {
    match s {
        "unverified" => Some(TrustLevel::Unverified),
        "community" => Some(TrustLevel::Community),
        "verified" => Some(TrustLevel::Verified),
        _ => None,
    }
}

fn trust_to_str(t: TrustLevel) -> &'static str {
    match t {
        TrustLevel::Unverified => "unverified",
        TrustLevel::Community => "community",
        TrustLevel::Verified => "verified",
    }
}

fn map_entry_to_core(e: ServerEntry) -> CoreServerEntry {
    let install_spec_json = serde_json::to_string(&e.install).unwrap_or_else(|_| "{}".into());
    let requirements_json = serde_json::to_string(&e.requirements).unwrap_or_else(|_| "[]".into());
    let default_env_json = serde_json::to_string(&e.default_env).unwrap_or_else(|_| "[]".into());
    CoreServerEntry {
        id: e.id,
        source: e.source,
        display_name: e.display_name,
        summary: e.summary,
        description: e.description,
        categories: e.categories,
        tags: e.tags,
        author: e.author,
        homepage: e.homepage,
        version: e.version,
        trust: trust_to_str(e.trust).into(),
        icon: e.icon,
        install_spec_json,
        requirements_json,
        default_env_json,
    }
}

fn map_install_request(r: CoreInstallRequest) -> McpInstallRequest {
    McpInstallRequest {
        catalog_id: r.catalog_id,
        source: r.source,
        server_id_override: r.server_id_override,
        env_overrides: r.env_overrides,
        trust_grant: r.trust_grant,
        auto_start: r.auto_start,
    }
}

fn map_outcome_to_core(outcome: InstallOutcomeView) -> CoreInstallOutcomeView {
    match outcome {
        InstallOutcomeView::Installed { server_id, started } => CoreInstallOutcomeView {
            kind: "installed".into(),
            server_id: Some(server_id),
            started: Some(started),
            missing_runtimes: Vec::new(),
            missing_env_keys: Vec::new(),
        },
        InstallOutcomeView::RuntimeMissing { missing } => CoreInstallOutcomeView {
            kind: "runtime_missing".into(),
            server_id: None,
            started: None,
            missing_runtimes: missing
                .into_iter()
                .map(|r| r.kind.as_str().into())
                .collect(),
            missing_env_keys: Vec::new(),
        },
        InstallOutcomeView::AlreadyInstalled { server_id } => CoreInstallOutcomeView {
            kind: "already_installed".into(),
            server_id: Some(server_id),
            started: None,
            missing_runtimes: Vec::new(),
            missing_env_keys: Vec::new(),
        },
        InstallOutcomeView::InvalidEnv { missing_keys } => CoreInstallOutcomeView {
            kind: "invalid_env".into(),
            server_id: None,
            started: None,
            missing_runtimes: Vec::new(),
            missing_env_keys: missing_keys,
        },
    }
}

fn build_server_def(entry: &ServerEntry, req: &McpInstallRequest) -> McpServerDef {
    let server_id = req
        .server_id_override
        .clone()
        .unwrap_or_else(|| entry.id.clone());

    let mut env: std::collections::HashMap<String, String> = entry
        .default_env
        .iter()
        .filter_map(|spec| spec.default.clone().map(|v| (spec.key.clone(), v)))
        .collect();
    for (k, v) in &req.env_overrides {
        env.insert(k.clone(), v.clone());
    }

    let (transport, args) = match &entry.install {
        InstallSpec::Stdio {
            command,
            args,
            env: spec_env,
            cwd,
        } => {
            for (k, v) in spec_env {
                env.entry(k.clone()).or_insert_with(|| v.clone());
            }
            (
                McpTransportDef::Stdio {
                    command: command.clone(),
                    cwd: cwd.clone(),
                },
                args.clone(),
            )
        }
        InstallSpec::Sse { url, headers } => (
            McpTransportDef::Sse {
                url: url.clone(),
                api_key_env: None,
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            },
            Vec::new(),
        ),
    };

    McpServerDef {
        name: server_id,
        transport,
        args,
        env,
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: true,
        max_restart_attempts: 3,
    }
}

fn emit_marketplace_event(tx: &tokio::sync::broadcast::Sender<DomainEvent>, payload: EventPayload) {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        payload,
    );
    let _ = tx.send(event);
}

/// View descriptor for the always-present implicit "builtin" catalog
/// source. Returned by [`AppFacade::list_catalog_sources`] both when the
/// marketplace is fully wired and when it is not configured at all.
fn builtin_source_view() -> CatalogSourceView {
    CatalogSourceView {
        id: "builtin".into(),
        display_name: "Built-in".into(),
        kind: "builtin".into(),
        url: String::new(),
        api_key_env: None,
        priority: 0,
        default_trust: "verified".into(),
        enabled: true,
        cache_ttl_seconds: None,
        last_error: None,
    }
}

/// Build a builtin-only `AggregateCatalogProvider` for the degraded path
/// where the user has no `[mcp_marketplace]` section in `kairox.toml`.
fn builtin_only_provider() -> agent_core::Result<Arc<agent_mcp::catalog::AggregateCatalogProvider>>
{
    static BUILTIN_AGGREGATE: std::sync::OnceLock<
        Arc<agent_mcp::catalog::AggregateCatalogProvider>,
    > = std::sync::OnceLock::new();
    let agg = BUILTIN_AGGREGATE.get_or_init(|| {
        let builtin = Arc::new(
            agent_mcp::catalog::BuiltinCatalogProvider::new()
                .expect("BUILTIN_JSON must parse; this is a build-time invariant"),
        );
        let providers: Vec<Arc<dyn CatalogProvider>> = vec![builtin];
        Arc::new(agent_mcp::catalog::AggregateCatalogProvider::new(providers))
    });
    Ok(Arc::clone(agg))
}

fn catalog_source_to_view(s: agent_config::CatalogSourceConfig) -> CatalogSourceView {
    CatalogSourceView {
        id: s.id,
        display_name: s.display_name,
        kind: match s.kind {
            agent_config::CatalogSourceKind::McpRegistry => "mcp_registry".into(),
        },
        url: s.url,
        api_key_env: s.api_key_env,
        priority: s.priority,
        default_trust: s.default_trust,
        enabled: s.enabled,
        cache_ttl_seconds: s.cache_ttl_seconds,
        last_error: None,
    }
}

fn request_to_source_config(
    r: AddCatalogSourceRequest,
) -> agent_core::Result<agent_config::CatalogSourceConfig> {
    let kind = match r.kind.as_str() {
        "mcp_registry" => agent_config::CatalogSourceKind::McpRegistry,
        other => {
            return Err(agent_core::CoreError::InvalidState(format!(
                "unsupported catalog source kind: {other}"
            )));
        }
    };
    if !r.url.starts_with("http://") && !r.url.starts_with("https://") {
        return Err(agent_core::CoreError::InvalidState(
            "url must start with http:// or https://".into(),
        ));
    }
    Ok(agent_config::CatalogSourceConfig {
        id: r.id,
        display_name: r.display_name,
        kind,
        url: r.url,
        api_key_env: r.api_key_env,
        priority: r.priority.unwrap_or(100),
        default_trust: r.default_trust.unwrap_or_else(|| "community".into()),
        enabled: r.enabled.unwrap_or(true),
        cache_ttl_seconds: r.cache_ttl_seconds,
    })
}
