use crate::facade_runtime::LocalRuntime;
use agent_core::facade::{
    McpFacade, McpServerSettingsInput, McpServerSettingsTransport, McpServerSettingsView,
    ProfileSettingsInput, ProfileSettingsView,
};
use agent_core::{
    AddCatalogSourceRequest, CatalogQuery as CoreCatalogQuery, CatalogSourceView,
    InstallOutcomeView as CoreInstallOutcomeView, InstallRequest as CoreInstallRequest,
    InstalledEntry as CoreInstalledEntry, ServerEntry as CoreServerEntry,
};
use agent_mcp::{McpServerDef, McpTransportDef};
use agent_store::EventStore;
use async_trait::async_trait;

// ── MCP settings inherent methods ─────────────────────────────────────────

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) async fn list_mcp_server_settings(
        &self,
        source_filter: Option<String>,
    ) -> agent_core::Result<Vec<McpServerSettingsView>> {
        self.list_mcp_server_settings_for_project(source_filter, None)
            .await
    }

    pub(crate) async fn list_mcp_server_settings_for_project(
        &self,
        source_filter: Option<String>,
        project_root: Option<String>,
    ) -> agent_core::Result<Vec<McpServerSettingsView>> {
        let user_config_path = std::env::var("HOME").ok().map(|h| {
            std::path::PathBuf::from(h)
                .join(".kairox")
                .join("config.toml")
        });
        let project_config_path = project_root
            .map(std::path::PathBuf::from)
            .map(|root| root.join(".kairox").join("config.toml"))
            .or_else(|| {
                std::env::current_dir()
                    .ok()
                    .map(|d| d.join(".kairox").join("config.toml"))
            });
        let config = self.config();
        crate::mcp_settings::list_mcp_server_settings(
            &config,
            user_config_path.as_deref(),
            project_config_path.as_deref(),
            source_filter.as_deref(),
            self.mcp_manager.clone(),
        )
        .await
    }

    pub(crate) async fn upsert_mcp_server_settings(
        &self,
        input: McpServerSettingsInput,
    ) -> agent_core::Result<McpServerSettingsView> {
        let server_id = input.name.clone();
        let server_def = input
            .enabled
            .then(|| server_def_from_settings_input(&input));
        let config_path =
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?
                .ok_or_else(|| {
                    agent_core::CoreError::InvalidState(
                        "marketplace install dir not configured; cannot write MCP settings".into(),
                    )
                })?;
        let view = crate::mcp_settings::upsert_mcp_server_settings(&config_path, input).await?;
        self.sync_mcp_server_settings_registration(&server_id, server_def)
            .await?;
        Ok(view)
    }

    async fn sync_mcp_server_settings_registration(
        &self,
        server_id: &str,
        server_def: Option<McpServerDef>,
    ) -> agent_core::Result<()> {
        let Some(manager) = &self.mcp_manager else {
            if server_def.is_some() {
                tracing::warn!(
                    "MCP settings update saved {server_id}, but no MCP manager is configured"
                );
            }
            return Ok(());
        };

        let mut manager = manager.lock().await;
        manager
            .unregister_dynamic(server_id)
            .await
            .map_err(|error| {
                agent_core::CoreError::InvalidState(format!(
                    "failed to update MCP server registration: {error}"
                ))
            })?;
        if let Some(def) = server_def {
            manager.register_dynamic(def).map_err(|error| {
                agent_core::CoreError::InvalidState(format!(
                    "failed to update MCP server registration: {error}"
                ))
            })?;
        }
        Ok(())
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
}

fn server_def_from_settings_input(input: &McpServerSettingsInput) -> McpServerDef {
    let (transport, args, env) = match &input.transport {
        McpServerSettingsTransport::Stdio { command, args, env } => (
            McpTransportDef::Stdio {
                command: command.clone(),
                cwd: None,
            },
            args.clone(),
            env.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        ),
        McpServerSettingsTransport::Sse { url, headers } => (
            McpTransportDef::Sse {
                url: url.clone(),
                api_key_env: None,
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            },
            Vec::new(),
            Default::default(),
        ),
        McpServerSettingsTransport::StreamableHttp { url, headers } => (
            McpTransportDef::StreamableHttp {
                url: url.clone(),
                api_key_env: None,
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            },
            Vec::new(),
            Default::default(),
        ),
    };

    McpServerDef {
        name: input.name.clone(),
        transport,
        args,
        env,
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: true,
        max_restart_attempts: 3,
    }
}

// ── McpFacade trait impl ──────────────────────────────────────────────────
//
// Methods are split across three files by concern:
//   facade_mcp      — MCP server settings (above)
//   facade_profiles — profile settings
//   facade_marketplace — marketplace catalog + catalog sources

#[async_trait]
impl<S, M> McpFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    // ── MCP Settings ──────────────────────────────────────────────────

    async fn list_mcp_server_settings(
        &self,
        source_filter: Option<String>,
    ) -> agent_core::Result<Vec<McpServerSettingsView>> {
        LocalRuntime::list_mcp_server_settings(self, source_filter).await
    }

    async fn list_mcp_server_settings_for_project(
        &self,
        source_filter: Option<String>,
        project_root: Option<String>,
    ) -> agent_core::Result<Vec<McpServerSettingsView>> {
        LocalRuntime::list_mcp_server_settings_for_project(self, source_filter, project_root).await
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

    // ── Profile Settings ──────────────────────────────────────────────

    async fn list_profile_settings(
        &self,
        source_filter: Option<String>,
    ) -> agent_core::Result<Vec<ProfileSettingsView>> {
        LocalRuntime::list_profile_settings(self, source_filter).await
    }

    async fn list_profile_settings_for_project(
        &self,
        source_filter: Option<String>,
        project_root: Option<String>,
    ) -> agent_core::Result<Vec<ProfileSettingsView>> {
        LocalRuntime::list_profile_settings_for_project(self, source_filter, project_root).await
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

    async fn open_profiles_config_file(&self) -> agent_core::Result<Option<String>> {
        LocalRuntime::open_profiles_config_file(self).await
    }

    // ── Marketplace Catalog ───────────────────────────────────────────

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

    // ── Catalog Sources ───────────────────────────────────────────────

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

#[cfg(test)]
#[path = "facade_mcp_tests.rs"]
mod tests;
