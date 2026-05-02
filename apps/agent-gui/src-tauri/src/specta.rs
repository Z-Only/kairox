//! specta bindings — auto-generated TypeScript types from Tauri commands.
//!
//! Run `just gen-types` to regenerate the TypeScript bindings.

use crate::commands::*;
use tauri_specta::collect_commands;

/// Build the specta collector with all command type information.
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
        ])
        .typ::<WorkspaceInfoResponse>()
        .typ::<SessionInfoResponse>()
        .typ::<MemoryEntryResponse>()
}
