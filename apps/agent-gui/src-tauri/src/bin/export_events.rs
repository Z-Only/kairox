//! Binary to export event-related TypeScript types via specta.
//!
//! Usage: cargo run -p agent-gui-tauri --bin export-events
//!
//! Output: apps/agent-gui/src/generated/events.ts

use agent_core::{
    AgentRole, DomainEvent, EventPayload, PrivacyClassification, TaskGraphSnapshot, TaskSnapshot,
    TaskState,
};
use agent_memory::MemoryScope;

fn main() {
    let out_path_str = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "../../src/generated/events.ts".to_string());
    let out_path = std::path::Path::new(&out_path_str);

    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).expect("Failed to create output directory");
    }

    let specta_builder = tauri_specta::Builder::<tauri::Wry>::new()
        .typ::<EventPayload>()
        .typ::<DomainEvent>()
        .typ::<PrivacyClassification>()
        .typ::<AgentRole>()
        .typ::<TaskState>()
        .typ::<TaskSnapshot>()
        .typ::<TaskGraphSnapshot>()
        .typ::<MemoryScope>();

    specta_builder
        .export(specta_typescript::Typescript::default(), out_path)
        .expect("Failed to export event types");

    eprintln!("Event types exported to {}", out_path.display());
}
