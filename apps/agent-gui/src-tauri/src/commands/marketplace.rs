use super::*;

#[derive(Debug, Clone, Default, Serialize, Deserialize, specta::Type)]
pub struct CatalogQueryRequest {
    pub keyword: Option<String>,
    pub category: Option<String>,
    /// "unverified" | "community" | "verified"
    pub trust_min: Option<String>,
    pub source: Option<String>,
    #[specta(type = u32)]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ServerEntryResponse {
    pub id: String,
    pub source: String,
    pub display_name: String,
    pub summary: String,
    pub description: String,
    pub categories: Vec<String>,
    pub tags: Vec<String>,
    pub author: Option<String>,
    pub homepage: Option<String>,
    pub version: Option<String>,
    /// Lower-case trust level: "unverified" | "community" | "verified".
    pub trust: String,
    pub icon: Option<String>,
    /// JSON-encoded `agent_mcp::catalog::InstallSpec`.
    pub install_spec_json: String,
    /// JSON-encoded `Vec<agent_mcp::catalog::RuntimeRequirement>`.
    pub requirements_json: String,
    /// JSON-encoded `Vec<agent_mcp::catalog::EnvVarSpec>`.
    pub default_env_json: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct InstallRequestPayload {
    pub catalog_id: String,
    pub source: String,
    pub server_id_override: Option<String>,
    pub env_overrides: std::collections::BTreeMap<String, String>,
    pub trust_grant: bool,
    pub auto_start: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct InstallOutcomeResponse {
    /// "installed" | "runtime_missing" | "already_installed" | "invalid_env"
    pub kind: String,
    pub server_id: Option<String>,
    pub started: Option<bool>,
    pub missing_runtimes: Vec<String>,
    pub missing_env_keys: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct InstalledEntryResponse {
    pub server_id: String,
    pub catalog_id: Option<String>,
    pub source: Option<String>,
    pub display_name: String,
    pub installed_at: String,
    pub running: bool,
}

// ---------------------------------------------------------------------------
// Marketplace commands
// ---------------------------------------------------------------------------

#[tauri::command]
#[specta::specta]
pub async fn list_catalog(
    state: State<'_, GuiState>,
    query: Option<CatalogQueryRequest>,
) -> Result<Vec<ServerEntryResponse>, String> {
    let q = into_core_query(query.unwrap_or_default());
    let entries = state
        .runtime
        .list_catalog(q)
        .await
        .map_err(|e| e.to_string())?;
    Ok(entries.into_iter().map(into_response_entry).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn get_catalog_entry(
    state: State<'_, GuiState>,
    id: String,
    source: Option<String>,
) -> Result<Option<ServerEntryResponse>, String> {
    let e = state
        .runtime
        .get_catalog_entry(id, source)
        .await
        .map_err(|e| e.to_string())?;
    Ok(e.map(into_response_entry))
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_catalog(
    state: State<'_, GuiState>,
    source: Option<String>,
) -> Result<(), String> {
    state
        .runtime
        .refresh_catalog(source)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn install_catalog_entry(
    state: State<'_, GuiState>,
    request: InstallRequestPayload,
) -> Result<InstallOutcomeResponse, String> {
    let outcome = state
        .runtime
        .install_catalog_entry(into_core_install_request(request))
        .await
        .map_err(|e| e.to_string())?;
    Ok(into_response_outcome(outcome))
}

#[tauri::command]
#[specta::specta]
pub async fn uninstall_catalog_entry(
    state: State<'_, GuiState>,
    server_id: String,
) -> Result<(), String> {
    state
        .runtime
        .uninstall_catalog_entry(server_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn list_installed_entries(
    state: State<'_, GuiState>,
) -> Result<Vec<InstalledEntryResponse>, String> {
    let v = state
        .runtime
        .list_installed_entries()
        .await
        .map_err(|e| e.to_string())?;
    Ok(v.into_iter()
        .map(|e| InstalledEntryResponse {
            server_id: e.server_id,
            catalog_id: e.catalog_id,
            source: e.source,
            display_name: e.display_name,
            installed_at: e.installed_at,
            running: e.running,
        })
        .collect())
}

// ---------------------------------------------------------------------------
// Marketplace helper conversions
// ---------------------------------------------------------------------------

fn into_core_query(q: CatalogQueryRequest) -> agent_core::CatalogQuery {
    agent_core::CatalogQuery {
        keyword: q.keyword,
        category: q.category,
        trust_min: q.trust_min,
        source: q.source,
        limit: q.limit,
    }
}

fn into_response_entry(e: agent_core::ServerEntry) -> ServerEntryResponse {
    ServerEntryResponse {
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
        trust: e.trust,
        icon: e.icon,
        install_spec_json: e.install_spec_json,
        requirements_json: e.requirements_json,
        default_env_json: e.default_env_json,
    }
}

fn into_core_install_request(p: InstallRequestPayload) -> agent_core::InstallRequest {
    agent_core::InstallRequest {
        catalog_id: p.catalog_id,
        source: p.source,
        server_id_override: p.server_id_override,
        env_overrides: p.env_overrides,
        trust_grant: p.trust_grant,
        auto_start: p.auto_start,
    }
}

fn into_response_outcome(o: agent_core::InstallOutcomeView) -> InstallOutcomeResponse {
    InstallOutcomeResponse {
        kind: o.kind,
        server_id: o.server_id,
        started: o.started,
        missing_runtimes: o.missing_runtimes,
        missing_env_keys: o.missing_env_keys,
    }
}

#[cfg(test)]
mod marketplace_command_tests {
    use super::*;

    #[test]
    fn install_outcome_response_serializes_kind_string() {
        let r = InstallOutcomeResponse {
            kind: "installed".into(),
            server_id: Some("filesystem".into()),
            started: Some(true),
            missing_runtimes: vec![],
            missing_env_keys: vec![],
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"kind\":\"installed\""));
        assert!(json.contains("\"server_id\":\"filesystem\""));
    }

    #[test]
    fn catalog_query_request_default_is_all_none() {
        let q = CatalogQueryRequest::default();
        assert!(q.keyword.is_none() && q.category.is_none() && q.trust_min.is_none());
    }
}

// ---------------------------------------------------------------------------
// Phase 2: catalog source commands + types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct CatalogSourceViewResponse {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub url: String,
    pub api_key_env: Option<String>,
    pub priority: u32,
    pub default_trust: String,
    pub enabled: bool,
    #[specta(type = u32)]
    pub cache_ttl_seconds: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct AddCatalogSourceRequestPayload {
    pub id: String,
    pub display_name: String,
    pub kind: String,
    pub url: String,
    pub api_key_env: Option<String>,
    pub priority: Option<u32>,
    pub default_trust: Option<String>,
    pub enabled: Option<bool>,
    #[specta(type = u32)]
    pub cache_ttl_seconds: Option<u64>,
}

#[tauri::command]
#[specta::specta]
pub async fn list_catalog_sources(
    state: State<'_, GuiState>,
) -> Result<Vec<CatalogSourceViewResponse>, String> {
    let v = state
        .runtime
        .list_catalog_sources()
        .await
        .map_err(|e| e.to_string())?;
    Ok(v.into_iter().map(into_source_view_response).collect())
}

#[tauri::command]
#[specta::specta]
pub async fn add_catalog_source(
    state: State<'_, GuiState>,
    request: AddCatalogSourceRequestPayload,
) -> Result<(), String> {
    state
        .runtime
        .add_catalog_source(into_core_add_catalog_source_request(request))
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn remove_catalog_source(state: State<'_, GuiState>, id: String) -> Result<(), String> {
    state
        .runtime
        .remove_catalog_source(id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn set_catalog_source_enabled(
    state: State<'_, GuiState>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    state
        .runtime
        .set_catalog_source_enabled(id, enabled)
        .await
        .map_err(|e| e.to_string())
}

fn into_source_view_response(s: agent_core::CatalogSourceView) -> CatalogSourceViewResponse {
    CatalogSourceViewResponse {
        id: s.id,
        display_name: s.display_name,
        kind: s.kind,
        url: s.url,
        api_key_env: s.api_key_env,
        priority: s.priority,
        default_trust: s.default_trust,
        enabled: s.enabled,
        cache_ttl_seconds: s.cache_ttl_seconds,
        last_error: s.last_error,
    }
}

fn into_core_add_catalog_source_request(
    p: AddCatalogSourceRequestPayload,
) -> agent_core::AddCatalogSourceRequest {
    agent_core::AddCatalogSourceRequest {
        id: p.id,
        display_name: p.display_name,
        kind: p.kind,
        url: p.url,
        api_key_env: p.api_key_env,
        priority: p.priority,
        default_trust: p.default_trust,
        enabled: p.enabled,
        cache_ttl_seconds: p.cache_ttl_seconds,
    }
}

#[cfg(test)]
mod catalog_sources_command_tests {
    use super::*;

    #[test]
    fn source_view_response_serializes_kind_and_last_error() {
        let r = CatalogSourceViewResponse {
            id: "smithery".into(),
            display_name: "Smithery".into(),
            kind: "smithery".into(),
            url: "https://x".into(),
            api_key_env: None,
            priority: 50,
            default_trust: "community".into(),
            enabled: true,
            cache_ttl_seconds: None,
            last_error: Some("timeout".into()),
        };
        let json = serde_json::to_string(&r).unwrap();
        assert!(json.contains("\"kind\":\"smithery\""));
        assert!(json.contains("\"last_error\":\"timeout\""));
    }
}
