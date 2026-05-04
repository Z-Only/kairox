//! Binary to export specta TypeScript bindings.
//!
//! Usage: cargo run -p agent-gui-tauri --bin export-specta
//!
//! Output: apps/agent-gui/src/generated/commands.ts

use agent_config::ProfileInfo;
use agent_gui_tauri::commands::{
    BuildInfoResponse, MemoryEntryResponse, ProfileDetailResponse, SessionInfoResponse,
    TaskSnapshotResponse, WorkspaceInfoResponse,
};
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
        ])
        .typ::<WorkspaceInfoResponse>()
        .typ::<SessionInfoResponse>()
        .typ::<MemoryEntryResponse>()
        .typ::<ProfileInfo>()
        .typ::<ProfileDetailResponse>()
        .typ::<TaskSnapshotResponse>()
        .typ::<BuildInfoResponse>();

    specta_builder
        .export(specta_typescript::Typescript::default(), out_path)
        .expect("Failed to export specta types");

    eprintln!("TypeScript bindings exported to {}", out_path.display());
}
