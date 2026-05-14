//! Binary to export specta TypeScript bindings.
//!
//! Usage: cargo run -p agent-gui-tauri --bin export-specta
//!
//! Output: apps/agent-gui/src/generated/commands.ts

use agent_config::ProfileInfo;
use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, McpServerSettingsInput,
    McpServerSettingsTransport, McpServerSettingsView, ProfileSettingsInput, ProfileSettingsView,
    RemoteSkillSearchResult, SkillCatalogEntry, SkillCatalogQuery, SkillFieldMappingView,
    SkillInstallSource, SkillInstallTarget, SkillSettingsDetail, SkillSettingsScope,
    SkillSettingsView, SkillSourceView, SkillUpdateState,
};
use agent_core::{ActiveSkillView, SkillDetail, SkillView};
use agent_gui_tauri::commands::{
    AddCatalogSourceRequestPayload, BuildInfoResponse, CatalogQueryRequest,
    CatalogSourceViewResponse, ConnectivityTestResult, InstallOutcomeResponse,
    InstallRequestPayload, InstalledEntryResponse, McpContentBlockResponse, McpPromptDefResponse,
    McpResourceDefResponse, McpServerStatusResponse, McpToolDefResponse, MemoryEntryResponse,
    ProfileDetailResponse, ProjectGitStatusResponse, ProjectInfoResponse,
    ProjectInstructionSummaryResponse, SaveDraftRequest, ServerEntryResponse, SessionInfoResponse,
    TaskSnapshotResponse, WorkspaceInfoResponse,
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
            agent_gui_tauri::commands::list_profiles_with_limits,
            agent_gui_tauri::commands::get_profile_info,
            agent_gui_tauri::commands::refresh_config_for_project,
            agent_gui_tauri::commands::initialize_workspace,
            agent_gui_tauri::commands::start_session,
            agent_gui_tauri::commands::send_message,
            agent_gui_tauri::commands::list_sessions,
            // Project workspace commands
            agent_gui_tauri::commands::list_projects,
            agent_gui_tauri::commands::create_blank_project,
            agent_gui_tauri::commands::add_existing_project,
            agent_gui_tauri::commands::rename_project,
            agent_gui_tauri::commands::remove_project,
            agent_gui_tauri::commands::restore_project_session,
            agent_gui_tauri::commands::update_project_order,
            agent_gui_tauri::commands::update_project_expanded,
            agent_gui_tauri::commands::create_project_draft_session,
            agent_gui_tauri::commands::list_project_sessions,
            agent_gui_tauri::commands::list_archived_sessions,
            agent_gui_tauri::commands::create_project_worktree_session,
            agent_gui_tauri::commands::get_project_git_status,
            agent_gui_tauri::commands::get_session_git_status,
            agent_gui_tauri::commands::init_project_git,
            agent_gui_tauri::commands::get_project_instruction_summary,
            agent_gui_tauri::commands::resolve_permission,
            agent_gui_tauri::commands::query_memories,
            agent_gui_tauri::commands::delete_memory,
            agent_gui_tauri::commands::list_workspaces,
            agent_gui_tauri::commands::rename_session,
            agent_gui_tauri::commands::delete_session,
            agent_gui_tauri::commands::permanently_delete_session,
            agent_gui_tauri::commands::restore_archived_session,
            agent_gui_tauri::commands::get_profile_detail,
            agent_gui_tauri::commands::restore_workspace,
            agent_gui_tauri::commands::get_task_graph,
            agent_gui_tauri::commands::cancel_session,
            agent_gui_tauri::commands::compact_session,
            agent_gui_tauri::commands::switch_model,
            agent_gui_tauri::commands::get_permission_mode,
            agent_gui_tauri::commands::get_build_info,
            // Skill commands
            agent_gui_tauri::commands::list_skills,
            agent_gui_tauri::commands::get_skill_detail,
            agent_gui_tauri::commands::activate_skill,
            agent_gui_tauri::commands::deactivate_skill,
            agent_gui_tauri::commands::list_active_skills,
            // Settings commands
            agent_gui_tauri::commands::list_mcp_server_settings,
            agent_gui_tauri::commands::upsert_mcp_server_settings,
            agent_gui_tauri::commands::set_mcp_server_enabled,
            agent_gui_tauri::commands::delete_mcp_server_settings,
            agent_gui_tauri::commands::open_mcp_config_file,
            // Profile settings commands
            agent_gui_tauri::commands::list_profile_settings,
            agent_gui_tauri::commands::upsert_profile_settings,
            agent_gui_tauri::commands::set_profile_enabled,
            agent_gui_tauri::commands::delete_profile_settings,
            agent_gui_tauri::commands::move_profile_in_order,
            agent_gui_tauri::commands::test_model_connectivity,
            agent_gui_tauri::commands::test_url_connectivity,
            agent_gui_tauri::commands::open_config_dir,
            agent_gui_tauri::commands::open_profiles_config_file,
            agent_gui_tauri::commands::open_skills_dir,
            agent_gui_tauri::commands::list_skill_settings,
            agent_gui_tauri::commands::get_skill_settings_detail,
            agent_gui_tauri::commands::set_skill_enabled,
            agent_gui_tauri::commands::delete_skill_settings,
            agent_gui_tauri::commands::search_remote_skills,
            agent_gui_tauri::commands::install_remote_skill,
            agent_gui_tauri::commands::install_github_skill,
            agent_gui_tauri::commands::update_skill,
            // Skill catalog commands
            agent_gui_tauri::commands::list_skill_catalog,
            agent_gui_tauri::commands::list_skill_sources,
            agent_gui_tauri::commands::add_skill_source,
            agent_gui_tauri::commands::remove_skill_source,
            agent_gui_tauri::commands::set_skill_source_enabled,
            agent_gui_tauri::commands::refresh_skill_catalog,
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
            agent_gui_tauri::commands::save_draft,
            agent_gui_tauri::commands::get_draft,
        ])
        .typ::<WorkspaceInfoResponse>()
        .typ::<SessionInfoResponse>()
        .typ::<ProjectInfoResponse>()
        .typ::<ProjectGitStatusResponse>()
        .typ::<ProjectInstructionSummaryResponse>()
        .typ::<MemoryEntryResponse>()
        .typ::<ProfileInfo>()
        .typ::<ProfileDetailResponse>()
        .typ::<TaskSnapshotResponse>()
        .typ::<BuildInfoResponse>()
        // Skill response types
        .typ::<SkillView>()
        .typ::<SkillDetail>()
        .typ::<ActiveSkillView>()
        // Settings request/response types
        .typ::<McpServerSettingsView>()
        .typ::<McpServerSettingsInput>()
        .typ::<McpServerSettingsTransport>()
        .typ::<ProfileSettingsView>()
        .typ::<ProfileSettingsInput>()
        .typ::<SkillSettingsView>()
        .typ::<SkillSettingsDetail>()
        .typ::<SkillSettingsScope>()
        .typ::<SkillInstallSource>()
        .typ::<SkillUpdateState>()
        .typ::<RemoteSkillSearchResult>()
        .typ::<SkillInstallTarget>()
        .typ::<InstallRemoteSkillRequest>()
        .typ::<InstallGithubSkillRequest>()
        // Skill catalog types
        .typ::<SkillCatalogEntry>()
        .typ::<SkillCatalogQuery>()
        .typ::<SkillSourceView>()
        .typ::<SkillFieldMappingView>()
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
        .typ::<AddCatalogSourceRequestPayload>()
        .typ::<ConnectivityTestResult>()
        // Draft persistence types
        .typ::<SaveDraftRequest>();

    match specta_builder.export(specta_typescript::Typescript::default(), out_path) {
        Ok(()) => eprintln!("TypeScript bindings exported to {}", out_path.display()),
        Err(e) => {
            eprintln!("Export error: {e:?}");
            std::process::exit(1);
        }
    }
}
