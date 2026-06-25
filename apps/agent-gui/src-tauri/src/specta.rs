//! specta bindings — auto-generated TypeScript types from Tauri commands.
//!
//! Run `just gen-types` to regenerate the TypeScript bindings.

use crate::commands::*;
use agent_core::facade::{
    AgentSettingsInput, AgentSettingsScope, AgentSettingsView, AutonomousTaskView, CheckpointView,
    EffectiveAgentView, EffectiveMcpServerView, EffectiveProfileView, EffectiveSkillView,
    HookSettingsInput, HookSettingsView, HookTemplateView, HooksSettingsView,
    InstallGithubSkillRequest, InstallPluginRequest, InstallRemoteSkillRequest,
    InstructionsUpdateInput, InstructionsView, McpServerSettingsInput, McpServerSettingsTransport,
    McpServerSettingsView, PluginCatalogEntry, PluginComponentInventoryView, PluginDetailView,
    PluginInstallTarget, PluginMarketplaceSourceView, PluginSettingsView, ProfileSettingsInput,
    ProfileSettingsView, RemoteSkillSearchResult, SkillCatalogEntry, SkillCatalogQuery,
    SkillFieldMappingView, SkillInstallSource, SkillInstallTarget, SkillSettingsDetail,
    SkillSettingsScope, SkillSettingsView, SkillSourceView, SkillUpdateState, TraceExport,
};
use agent_core::{
    ActiveSkillView, AdvisorConcern, AdvisorMode, AdvisorReview, AdvisorVerdict, AgentRole,
    CompactionReason, CompactionStatus, ConfigScope, ContextSource, ContextUsage, DomainEvent,
    EventPayload, PrivacyClassification, ProjectedModelLimits, SkillDetail, SkillView,
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
            refresh_config,
            refresh_config_for_project,
            get_gui_settings,
            set_gui_devtools_enabled,
            initialize_workspace,
            start_session,
            send_message,
            send_message_to_session,
            send_message_to_session_and_wait,
            send_message_to_session_if_idle,
            export_trace,
            export_session_diagnostics,
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
            list_project_branches,
            get_project_git_status,
            get_session_git_status,
            get_project_git_review,
            get_session_git_review,
            init_project_git,
            get_project_instruction_summary,
            resolve_permission,
            query_memories,
            accept_memory,
            reject_memory,
            delete_memory,
            list_workspaces,
            resolve_task_confirmation,
            rename_session,
            delete_session,
            permanently_delete_session,
            restore_archived_session,
            get_profile_detail,
            restore_workspace,
            get_task_graph,
            retry_task,
            cancel_task,
            cancel_session,
            compact_session,
            switch_model,
            get_session_approval_policy,
            set_session_approval_policy,
            get_session_sandbox_policy,
            set_session_sandbox_policy,
            get_build_info,
            // Skill commands
            list_skills,
            get_skill_detail,
            activate_skill,
            deactivate_skill,
            list_active_skills,
            // Settings commands
            list_mcp_server_settings,
            get_effective_mcp_servers,
            get_effective_skills,
            get_effective_model_profiles,
            upsert_mcp_server_settings,
            set_mcp_server_enabled,
            delete_mcp_server_settings,
            disable_mcp_server_at_scope,
            enable_mcp_server_at_scope,
            open_mcp_config_file,
            // Instructions settings commands
            get_instructions,
            upsert_instructions,
            get_system_prompt,
            get_hooks_settings,
            upsert_hook_settings,
            delete_hook_settings,
            // Profile settings commands
            list_profile_settings,
            upsert_profile_settings,
            set_profile_enabled,
            delete_profile_settings,
            move_profile_in_order,
            test_model_connectivity,
            test_url_connectivity,
            open_config_dir,
            open_profiles_config_file,
            open_config_file_for_scope,
            open_agents_dir,
            list_agent_settings,
            upsert_agent_settings,
            delete_agent_settings,
            copy_agent_settings,
            open_skills_dir,
            list_skill_settings,
            get_skill_settings_detail,
            set_skill_enabled,
            delete_skill_settings,
            search_remote_skills,
            install_remote_skill,
            install_github_skill,
            update_skill,
            // Skill catalog commands
            list_skill_catalog,
            list_skill_sources,
            add_skill_source,
            remove_skill_source,
            set_skill_source_enabled,
            refresh_skill_catalog,
            // Plugin commands
            list_plugin_settings,
            get_plugin_detail,
            set_plugin_enabled,
            delete_plugin_settings,
            list_plugin_marketplace_sources,
            set_plugin_marketplace_source_enabled,
            list_plugin_catalog,
            install_plugin,
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
            test_mcp_connectivity,
            check_mcp_health,
            set_mcp_tool_disabled,
            get_mcp_tool_states,
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
            // Monitor commands
            list_monitors,
            stop_monitor,
            list_workspace_files,
            save_draft,
            get_draft,
            // Trajectory commands
            list_trajectories,
            get_trajectory_steps,
            export_trajectory,
            // Autonomous task commands
            list_autonomous_tasks,
            get_autonomous_task,
            get_autonomous_checkpoints,
            pause_autonomous_task,
            resume_autonomous_task,
            cancel_autonomous_task,
        ])
        .typ::<WorkspaceInfoResponse>()
        .typ::<WorkspaceFilesResponse>()
        .typ::<SessionInfoResponse>()
        .typ::<SendMessageToSessionIfIdleResponse>()
        .typ::<ProjectInfoResponse>()
        .typ::<ProjectGitFileChangeResponse>()
        .typ::<ProjectGitDiffSectionResponse>()
        .typ::<ProjectGitReviewResponse>()
        .typ::<ProjectGitStatusResponse>()
        .typ::<ProjectInstructionSummaryResponse>()
        .typ::<MemoryEntryResponse>()
        .typ::<ProfileDetailResponse>()
        .typ::<ProfileWithLimits>()
        .typ::<TaskSnapshotResponse>()
        .typ::<TraceExport>()
        .typ::<SessionDiagnosticsResponse>()
        .typ::<EventTypeCountResponse>()
        .typ::<SessionDiagnosticsMessageResponse>()
        .typ::<ModelToolCallDiagnosticsResponse>()
        .typ::<McpToolCallDiagnosticsResponse>()
        .typ::<TrajectoryCompletedDiagnosticsResponse>()
        .typ::<BuildInfoResponse>()
        .typ::<GuiSettingsView>()
        .typ::<MonitorInfoResponse>()
        // Skill response types
        .typ::<SkillView>()
        .typ::<SkillDetail>()
        .typ::<ActiveSkillView>()
        // Effective config types
        .typ::<ConfigScope>()
        .typ::<EffectiveMcpServerView>()
        .typ::<EffectiveAgentView>()
        .typ::<EffectiveSkillView>()
        .typ::<EffectiveProfileView>()
        // Settings request/response types
        .typ::<McpServerSettingsView>()
        .typ::<McpServerSettingsInput>()
        .typ::<McpServerSettingsTransport>()
        .typ::<ProfileSettingsView>()
        .typ::<ProfileSettingsInput>()
        .typ::<AgentSettingsScope>()
        .typ::<AgentSettingsView>()
        .typ::<AgentSettingsInput>()
        .typ::<InstructionsView>()
        .typ::<InstructionsUpdateInput>()
        .typ::<HookSettingsInput>()
        .typ::<HookSettingsView>()
        .typ::<HookTemplateView>()
        .typ::<HooksSettingsView>()
        .typ::<SkillSettingsView>()
        .typ::<SkillSettingsDetail>()
        .typ::<SkillSettingsScope>()
        .typ::<SkillInstallSource>()
        .typ::<SkillUpdateState>()
        .typ::<RemoteSkillSearchResult>()
        .typ::<SkillInstallTarget>()
        .typ::<InstallRemoteSkillRequest>()
        .typ::<InstallGithubSkillRequest>()
        .typ::<PluginSettingsView>()
        .typ::<PluginDetailView>()
        .typ::<PluginComponentInventoryView>()
        .typ::<PluginMarketplaceSourceView>()
        .typ::<PluginCatalogEntry>()
        .typ::<InstallPluginRequest>()
        .typ::<PluginInstallTarget>()
        // MCP response types
        .typ::<McpServerStatusResponse>()
        .typ::<McpToolDefResponse>()
        .typ::<McpResourceDefResponse>()
        .typ::<McpPromptDefResponse>()
        .typ::<McpContentBlockResponse>()
        .typ::<McpServerStatus>()
        .typ::<agent_mcp::ConnectivityResult>()
        // Marketplace request/response types
        .typ::<CatalogQueryRequest>()
        .typ::<ServerEntryResponse>()
        .typ::<InstallRequestPayload>()
        .typ::<InstallOutcomeResponse>()
        .typ::<InstalledEntryResponse>()
        // Skill catalog types
        .typ::<SkillCatalogEntry>()
        .typ::<SkillCatalogQuery>()
        .typ::<SkillSourceView>()
        .typ::<SkillFieldMappingView>()
        // Phase 2: catalog source types
        .typ::<CatalogSourceViewResponse>()
        .typ::<AddCatalogSourceRequestPayload>()
        .typ::<ConnectivityTestResult>()
        // Draft persistence types
        .typ::<SaveDraftRequest>()
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
        // Trajectory types
        .typ::<TrajectoryMetaResponse>()
        .typ::<TrajectoryStepResponse>()
        // Autonomous task types
        .typ::<AutonomousTaskView>()
        .typ::<CheckpointView>()
        // Advisor (self-reflection) types
        .typ::<AdvisorMode>()
        .typ::<AdvisorVerdict>()
        .typ::<AdvisorConcern>()
        .typ::<AdvisorReview>()
}
