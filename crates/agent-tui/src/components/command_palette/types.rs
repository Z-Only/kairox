//! Data types for the command palette — data-adjacent free functions that
//! don't depend on the [`CommandPalette`] struct itself.
//!
//! These are extracted from `state.rs` so that the main state module stays
//! focused on the `CommandPalette` struct and its query/navigation methods.

use crate::components::{EventContext, ModelProfileEntry};

// ---------------------------------------------------------------------------
// Data-adjacent free functions (no CommandPalette dependency)
// ---------------------------------------------------------------------------

pub(super) fn active_project_id(ctx: &EventContext) -> Option<agent_core::ProjectId> {
    let session_id = ctx.current_session_id.as_ref()?;
    ctx.sessions
        .iter()
        .find(|session| &session.id == session_id)
        .and_then(|session| session.project_id.clone())
}

pub(super) fn model_profile_display(profile: &ModelProfileEntry) -> String {
    if !profile.provider_display.is_empty() && !profile.model_display.is_empty() {
        format!("{} / {}", profile.provider_display, profile.model_display)
    } else if !profile.model_display.is_empty() {
        profile.model_display.clone()
    } else {
        profile.alias.clone()
    }
}
