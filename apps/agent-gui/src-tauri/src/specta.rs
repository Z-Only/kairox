//! specta bindings — auto-generated TypeScript types from Tauri commands.
//!
//! Run `just gen-types` to regenerate the TypeScript bindings.

use crate::commands::*;
use agent_core::{
    AgentRole, DomainEvent, EventPayload, PrivacyClassification, TaskGraphSnapshot, TaskSnapshot,
    TaskState,
};
use agent_mcp::McpServerStatus;
use agent_memory::MemoryScope;
use tauri_specta::collect_commands;

/// Build the specta collector with all command and event type information.
pub fn create_specta() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new()
        .commands(collect_commands![
            list_profiles,
            get_profile_info,
            initialize_workspace,
            start_session,
            send_message,
            list_sessions,
            resolve_permission,
            query_memories,
            delete_memory,
            list_workspaces,
            rename_session,
            delete_session,
            get_profile_detail,
            restore_workspace,
            get_task_graph,
            cancel_session,
            get_permission_mode,
            get_build_info,
            // MCP commands
            list_mcp_servers,
            start_mcp_server,
            stop_mcp_server,
            refresh_mcp_tools,
            trust_mcp_server,
            revoke_mcp_trust,
            list_mcp_resources,
            list_mcp_prompts,
            read_mcp_resource,
            // Marketplace commands
            list_catalog,
            get_catalog_entry,
            refresh_catalog,
            install_catalog_entry,
            uninstall_catalog_entry,
            list_installed_entries,
        ])
        .typ::<WorkspaceInfoResponse>()
        .typ::<SessionInfoResponse>()
        .typ::<MemoryEntryResponse>()
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
        // Event domain types (also exported by export-events binary)
        .typ::<EventPayload>()
        .typ::<DomainEvent>()
        .typ::<PrivacyClassification>()
        .typ::<AgentRole>()
        .typ::<TaskState>()
        .typ::<TaskSnapshot>()
        .typ::<TaskGraphSnapshot>()
        .typ::<MemoryScope>()
}
