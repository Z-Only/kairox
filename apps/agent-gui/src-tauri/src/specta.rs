//! specta bindings — auto-generated TypeScript types from Tauri commands.
//!
//! Run `just gen-types` to regenerate the TypeScript bindings.

use crate::commands::*;
use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, McpServerSettingsInput,
    McpServerSettingsTransport, McpServerSettingsView, RemoteSkillSearchResult, SkillInstallSource,
    SkillInstallTarget, SkillSettingsDetail, SkillSettingsScope, SkillSettingsView,
    SkillUpdateState,
};
use agent_core::{
    ActiveSkillView, AgentRole, CompactionReason, CompactionStatus, ContextSource, ContextUsage,
    DomainEvent, EventPayload, PrivacyClassification, ProjectedModelLimits, SkillDetail, SkillView,
    TaskGraphSnapshot, TaskSnapshot, TaskState,
};
use agent_mcp::McpServerStatus;
use agent_memory::MemoryScope;
use agent_models::{LimitSource, ModelLimits};
use tauri_specta::collect_commands;

/// Build the specta collector with all command and event type information.
pub fn create_specta() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::new()
        .commands(collect_commands![
            list_profiles,
            list_profiles_with_limits,
            get_profile_info,
            refresh_config_for_project,
            initialize_workspace,
            start_session,
            send_message,
            list_sessions,
            // Project workspace commands
            list_projects,
            create_blank_project,
            add_existing_project,
            rename_project,
            remove_project,
            restore_project_session,
            update_project_order,
            update_project_expanded,
            create_project_draft_session,
            list_project_sessions,
            list_archived_sessions,
            create_project_worktree_session,
            get_project_git_status,
            get_session_git_status,
            init_project_git,
            get_project_instruction_summary,
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
            compact_session,
            switch_model,
            get_permission_mode,
            get_build_info,
            // Skill commands
            list_skills,
            get_skill_detail,
            activate_skill,
            deactivate_skill,
            list_active_skills,
            // Settings commands
            list_mcp_server_settings,
            upsert_mcp_server_settings,
            set_mcp_server_enabled,
            delete_mcp_server_settings,
            open_mcp_config_file,
            list_skill_settings,
            get_skill_settings_detail,
            set_skill_enabled,
            delete_skill_settings,
            search_remote_skills,
            install_remote_skill,
            install_github_skill,
            update_skill,
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
            // Phase 2: catalog source commands
            list_catalog_sources,
            add_catalog_source,
            remove_catalog_source,
            set_catalog_source_enabled,
        ])
        .typ::<WorkspaceInfoResponse>()
        .typ::<SessionInfoResponse>()
        .typ::<ProjectInfoResponse>()
        .typ::<ProjectGitStatusResponse>()
        .typ::<ProjectInstructionSummaryResponse>()
        .typ::<MemoryEntryResponse>()
        .typ::<ProfileDetailResponse>()
        .typ::<ProfileWithLimits>()
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
        .typ::<SkillSettingsView>()
        .typ::<SkillSettingsDetail>()
        .typ::<SkillSettingsScope>()
        .typ::<SkillInstallSource>()
        .typ::<SkillUpdateState>()
        .typ::<RemoteSkillSearchResult>()
        .typ::<SkillInstallTarget>()
        .typ::<InstallRemoteSkillRequest>()
        .typ::<InstallGithubSkillRequest>()
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
        // Event domain types (also exported by export-events binary)
        .typ::<EventPayload>()
        .typ::<DomainEvent>()
        .typ::<PrivacyClassification>()
        .typ::<AgentRole>()
        .typ::<TaskState>()
        .typ::<TaskSnapshot>()
        .typ::<TaskGraphSnapshot>()
        .typ::<MemoryScope>()
        // Context-mgmt P1: per-model window metadata + budget-driven assembly
        .typ::<ContextSource>()
        .typ::<ContextUsage>()
        .typ::<ModelLimits>()
        .typ::<LimitSource>()
        // Context-mgmt P2: compaction reason (referenced by 4 new EventPayload variants)
        .typ::<CompactionReason>()
        // Context-mgmt P3: projection types consumed by GUI
        .typ::<CompactionStatus>()
        .typ::<ProjectedModelLimits>()
}
