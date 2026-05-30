mod order;
mod row;
mod view;
mod write;

use agent_core::CoreError;
use toml_edit::DocumentMut;

// ── Re-exports: public API ─────────────────────────────────────────────────

pub use order::{move_profile_in_order, save_profile_display_order};
pub use view::{list_profile_settings, writable_profiles_config_path};
pub use write::{
    delete_profile_in_file, set_profile_enabled_in_file, upsert_profile_settings_in_file,
};

// ── Shared helpers ─────────────────────────────────────────────────────────

pub(super) fn parse_document(raw: &str) -> agent_core::Result<DocumentMut> {
    raw.parse::<DocumentMut>().map_err(|error| {
        CoreError::InvalidState(format!("failed to parse profiles config: {error}"))
    })
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
