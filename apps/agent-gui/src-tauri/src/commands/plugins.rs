use super::*;
use agent_core::facade::{
    InstallPluginRequest, PluginCatalogEntry, PluginDetailView, PluginMarketplaceSourceView,
    PluginSettingsView, PluginsFacade,
};

#[tauri::command]
#[specta::specta]
pub async fn list_plugin_settings(
    state: State<'_, GuiState>,
) -> Result<Vec<PluginSettingsView>, String> {
    state
        .runtime
        .list_plugin_settings()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn get_plugin_detail(
    state: State<'_, GuiState>,
    settings_id: String,
) -> Result<PluginDetailView, String> {
    state
        .runtime
        .get_plugin_detail(settings_id.clone())
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("Plugin not found: {settings_id}"))
}

#[tauri::command]
#[specta::specta]
pub async fn set_plugin_enabled(
    state: State<'_, GuiState>,
    settings_id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_plugin_enabled(settings_id, enabled)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_plugin_settings(
    state: State<'_, GuiState>,
    settings_id: String,
) -> Result<(), String> {
    state
        .runtime
        .delete_plugin_settings(settings_id)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_plugin_marketplace_sources(
    state: State<'_, GuiState>,
) -> Result<Vec<PluginMarketplaceSourceView>, String> {
    state
        .runtime
        .list_plugin_marketplace_sources()
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_plugin_marketplace_source_enabled(
    state: State<'_, GuiState>,
    source_id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_plugin_marketplace_source_enabled(source_id, enabled)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_plugin_catalog(
    state: State<'_, GuiState>,
    marketplace_id: Option<String>,
    keyword: Option<String>,
) -> Result<Vec<PluginCatalogEntry>, String> {
    state
        .runtime
        .list_plugin_catalog(marketplace_id, keyword)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn install_plugin(
    state: State<'_, GuiState>,
    request: InstallPluginRequest,
) -> Result<PluginSettingsView, String> {
    state
        .runtime
        .install_plugin(request)
        .await
        .map_err(|error| error.to_string())
}
