//! Plugin settings and marketplace facade DTOs.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "snake_case")]
pub enum PluginInstallTarget {
    User,
    Project,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PluginComponentInventoryView {
    pub skill_count: u32,
    pub skill_names: Vec<String>,
    pub mcp_server_count: u32,
    pub app_count: u32,
    pub agent_count: u32,
    pub hook_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PluginSecurityMetadataView {
    pub publisher: Option<String>,
    pub trust: Option<String>,
    pub signature: Option<String>,
    pub checksum: Option<String>,
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PluginSettingsView {
    pub settings_id: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub scope: crate::config_scope::ConfigScope,
    pub path: String,
    pub enabled: bool,
    pub install_source: Option<String>,
    pub marketplace: Option<String>,
    pub effective: bool,
    pub shadowed_by: Option<String>,
    pub valid: bool,
    pub validation_error: Option<String>,
    pub inventory: PluginComponentInventoryView,
    pub manifest_kind: String,
    pub security: PluginSecurityMetadataView,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PluginDetailView {
    pub view: PluginSettingsView,
    pub manifest_path: String,
    pub homepage: Option<String>,
    pub repository: Option<String>,
    pub license: Option<String>,
    pub keywords: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PluginMarketplaceSourceView {
    pub id: String,
    pub display_name: String,
    pub source: String,
    pub enabled: bool,
    pub builtin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct PluginCatalogEntry {
    pub marketplace_id: String,
    pub name: String,
    pub description: String,
    pub version: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct InstallPluginRequest {
    pub marketplace_id: String,
    pub plugin_name: String,
    pub target: PluginInstallTarget,
}

#[async_trait]
pub trait PluginsFacade: Send + Sync {
    async fn list_plugin_settings(&self) -> crate::Result<Vec<PluginSettingsView>> {
        Ok(Vec::new())
    }

    async fn get_plugin_detail(
        &self,
        settings_id: String,
    ) -> crate::Result<Option<PluginDetailView>> {
        let _ = settings_id;
        Ok(None)
    }

    async fn set_plugin_enabled(&self, settings_id: String, enabled: bool) -> crate::Result<()> {
        let _ = (settings_id, enabled);
        Err(crate::CoreError::InvalidState(
            "plugin settings not configured".into(),
        ))
    }

    async fn delete_plugin_settings(&self, settings_id: String) -> crate::Result<()> {
        let _ = settings_id;
        Err(crate::CoreError::InvalidState(
            "plugin settings not configured".into(),
        ))
    }

    async fn list_plugin_marketplace_sources(
        &self,
    ) -> crate::Result<Vec<PluginMarketplaceSourceView>> {
        Ok(Vec::new())
    }

    async fn set_plugin_marketplace_source_enabled(
        &self,
        source_id: String,
        enabled: bool,
    ) -> crate::Result<()> {
        let _ = (source_id, enabled);
        Err(crate::CoreError::InvalidState(
            "plugin marketplace settings not configured".into(),
        ))
    }

    async fn list_plugin_catalog(
        &self,
        marketplace_id: Option<String>,
        keyword: Option<String>,
    ) -> crate::Result<Vec<PluginCatalogEntry>> {
        let _ = (marketplace_id, keyword);
        Ok(Vec::new())
    }

    async fn install_plugin(
        &self,
        request: InstallPluginRequest,
    ) -> crate::Result<PluginSettingsView> {
        let _ = request;
        Err(crate::CoreError::InvalidState(
            "plugin install not supported".into(),
        ))
    }
}
