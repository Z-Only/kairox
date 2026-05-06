//! Binary to export specta TypeScript bindings.
//!
//! Usage: cargo run -p agent-gui-tauri --bin export-specta
//!
//! Output: apps/agent-gui/src/generated/commands.ts

use agent_config::ProfileInfo;
use agent_gui_tauri::commands::{
    AddCatalogSourceRequestPayload, BuildInfoResponse, CatalogQueryRequest,
    CatalogSourceViewResponse, InstallOutcomeResponse, InstallRequestPayload,
    InstalledEntryResponse, McpContentBlockResponse, McpPromptDefResponse, McpResourceDefResponse,
    McpServerStatusResponse, McpToolDefResponse, MemoryEntryResponse, ProfileDetailResponse,
    ServerEntryResponse, SessionInfoResponse, TaskSnapshotResponse, WorkspaceInfoResponse,
};
use agent_mcp::McpServerStatus;
use tauri_specta::collect_commands;

fn main() {
    let out_path_str = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../../src/generated/commands.ts".to_string());
    let out_path = std::path::Path::new(&out_path_str);

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create output directory");
    }

    let specta_builder = tauri_specta::Builder::new()
        .commands(collect_commands![
            agent_gui_tauri::commands::list_profiles,
            agent_gui_tauri::commands::get_profile_info,
            agent_gui_tauri::commands::initialize_workspace,
            agent_gui_tauri::commands::start_session,
            agent_gui_tauri::commands::send_message,
            agent_gui_tauri::commands::list_sessions,
            agent_gui_tauri::commands::resolve_permission,
            agent_gui_tauri::commands::query_memories,
            agent_gui_tauri::commands::delete_memory,
            agent_gui_tauri::commands::list_workspaces,
            agent_gui_tauri::commands::rename_session,
            agent_gui_tauri::commands::delete_session,
            agent_gui_tauri::commands::get_profile_detail,
            agent_gui_tauri::commands::restore_workspace,
            agent_gui_tauri::commands::get_task_graph,
            agent_gui_tauri::commands::cancel_session,
            agent_gui_tauri::commands::get_permission_mode,
            agent_gui_tauri::commands::get_build_info,
            // MCP commands
            agent_gui_tauri::commands::list_mcp_servers,
            agent_gui_tauri::commands::start_mcp_server,
            agent_gui_tauri::commands::stop_mcp_server,
            agent_gui_tauri::commands::refresh_mcp_tools,
            agent_gui_tauri::commands::trust_mcp_server,
            agent_gui_tauri::commands::revoke_mcp_trust,
            agent_gui_tauri::commands::list_mcp_resources,
            agent_gui_tauri::commands::list_mcp_prompts,
            agent_gui_tauri::commands::read_mcp_resource,
            // Marketplace commands
            agent_gui_tauri::commands::list_catalog,
            agent_gui_tauri::commands::get_catalog_entry,
            agent_gui_tauri::commands::refresh_catalog,
            agent_gui_tauri::commands::install_catalog_entry,
            agent_gui_tauri::commands::uninstall_catalog_entry,
            agent_gui_tauri::commands::list_installed_entries,
            // Phase 2: catalog source commands
            agent_gui_tauri::commands::list_catalog_sources,
            agent_gui_tauri::commands::add_catalog_source,
            agent_gui_tauri::commands::remove_catalog_source,
            agent_gui_tauri::commands::set_catalog_source_enabled,
        ])
        .typ::<WorkspaceInfoResponse>()
        .typ::<SessionInfoResponse>()
        .typ::<MemoryEntryResponse>()
        .typ::<ProfileInfo>()
        .typ::<ProfileDetailResponse>()
        .typ::<TaskSnapshotResponse>()
        .typ::<BuildInfoResponse>()
        // MCP response types
        .typ::<McpServerStatusResponse>()
        .typ::<McpToolDefResponse>()
        .typ::<McpResourceDefResponse>()
        .typ::<McpPromptDefResponse>()
        .typ::<McpContentBlockResponse>()
        .typ::<McpServerStatus>()
        // Marketplace request/response types
        .typ::<CatalogQueryRequest>()
        .typ::<ServerEntryResponse>()
        .typ::<InstallRequestPayload>()
        .typ::<InstallOutcomeResponse>()
        .typ::<InstalledEntryResponse>()
        // Phase 2: catalog source types
        .typ::<CatalogSourceViewResponse>()
        .typ::<AddCatalogSourceRequestPayload>();

    specta_builder
        .export(specta_typescript::Typescript::default(), out_path)
        .expect("Failed to export specta types");

    eprintln!("TypeScript bindings exported to {}", out_path.display());
}
