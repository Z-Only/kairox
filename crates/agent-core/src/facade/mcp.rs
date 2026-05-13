//! MCP / marketplace / profile sub-trait — all methods provide default implementations.

use crate::facade::{
    AddCatalogSourceRequest, CatalogQuery, CatalogSourceView, InstallOutcomeView, InstallRequest,
    InstalledEntry, McpServerSettingsInput, McpServerSettingsView, ProfileSettingsInput,
    ProfileSettingsView, ServerEntry,
};
use async_trait::async_trait;

#[async_trait]
pub trait McpFacade: Send + Sync {
    // ── MCP Settings ──────────────────────────────────────────────────────

    async fn list_mcp_server_settings(
        &self,
        source_filter: Option<String>,
    ) -> crate::Result<Vec<McpServerSettingsView>> {
        let _ = source_filter;
        Ok(Vec::new())
    }

    async fn upsert_mcp_server_settings(
        &self,
        input: McpServerSettingsInput,
    ) -> crate::Result<McpServerSettingsView> {
        let _ = input;
        Err(crate::CoreError::InvalidState(
            "MCP settings mutation not supported".into(),
        ))
    }

    async fn delete_mcp_server_settings(&self, server_id: String) -> crate::Result<()> {
        let _ = server_id;
        Err(crate::CoreError::InvalidState(
            "MCP settings deletion not supported".into(),
        ))
    }

    async fn set_mcp_server_enabled(&self, server_id: String, enabled: bool) -> crate::Result<()> {
        let _ = (server_id, enabled);
        Err(crate::CoreError::InvalidState(
            "MCP settings enablement not supported".into(),
        ))
    }

    async fn open_mcp_config_file(&self) -> crate::Result<Option<String>> {
        Ok(None)
    }

    // ── Profile Settings ──────────────────────────────────────────────────

    async fn list_profile_settings(
        &self,
        source_filter: Option<String>,
    ) -> crate::Result<Vec<ProfileSettingsView>> {
        let _ = source_filter;
        Ok(Vec::new())
    }

    async fn upsert_profile_settings(
        &self,
        input: ProfileSettingsInput,
    ) -> crate::Result<ProfileSettingsView> {
        let _ = input;
        Err(crate::CoreError::InvalidState(
            "profile settings mutation not supported".into(),
        ))
    }

    async fn set_profile_enabled(&self, alias: String, enabled: bool) -> crate::Result<()> {
        let _ = (alias, enabled);
        Err(crate::CoreError::InvalidState(
            "profile settings enablement not supported".into(),
        ))
    }

    async fn delete_profile_settings(&self, alias: String) -> crate::Result<()> {
        let _ = alias;
        Err(crate::CoreError::InvalidState(
            "profile settings deletion not supported".into(),
        ))
    }

    async fn move_profile_in_order(&self, alias: String, direction: i32) -> crate::Result<()> {
        let _ = (alias, direction);
        Err(crate::CoreError::InvalidState(
            "profile ordering not supported".into(),
        ))
    }

    async fn open_config_dir(&self) -> crate::Result<Option<String>> {
        Ok(None)
    }

    // ── Marketplace Catalog ───────────────────────────────────────────────

    async fn list_catalog(&self, query: CatalogQuery) -> crate::Result<Vec<ServerEntry>> {
        let _ = query;
        Ok(Vec::new())
    }

    async fn get_catalog_entry(
        &self,
        id: String,
        source: Option<String>,
    ) -> crate::Result<Option<ServerEntry>> {
        let _ = (id, source);
        Ok(None)
    }

    async fn refresh_catalog(&self, source: Option<String>) -> crate::Result<()> {
        let _ = source;
        Ok(())
    }

    async fn install_catalog_entry(
        &self,
        request: InstallRequest,
    ) -> crate::Result<InstallOutcomeView> {
        let _ = request;
        Ok(InstallOutcomeView {
            kind: "runtime_missing".into(),
            server_id: None,
            started: None,
            missing_runtimes: Vec::new(),
            missing_env_keys: Vec::new(),
        })
    }

    async fn uninstall_catalog_entry(&self, server_id: String) -> crate::Result<()> {
        let _ = server_id;
        Ok(())
    }

    async fn list_installed_entries(&self) -> crate::Result<Vec<InstalledEntry>> {
        Ok(Vec::new())
    }

    // ── Catalog Sources ───────────────────────────────────────────────────

    async fn list_catalog_sources(&self) -> crate::Result<Vec<CatalogSourceView>> {
        Ok(vec![CatalogSourceView {
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
        }])
    }

    async fn add_catalog_source(&self, request: AddCatalogSourceRequest) -> crate::Result<()> {
        let _ = request;
        Ok(())
    }

    async fn remove_catalog_source(&self, id: String) -> crate::Result<()> {
        let _ = id;
        Ok(())
    }

    async fn set_catalog_source_enabled(&self, id: String, enabled: bool) -> crate::Result<()> {
        let _ = (id, enabled);
        Ok(())
    }
}
