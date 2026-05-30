//! Legacy text-only view helper.
//!
//! **Note:** This module is superseded by the component renderers in
//! [`crate::components::chat::render_messages`], [`crate::components::sessions`],
//! [`crate::components::trace`], etc. It is retained for backwards compatibility
//! and simple string-based tests only.

use agent_core::projection::{ProjectedRole, SessionProjection};

#[allow(dead_code)]
pub fn render_lines(projection: &SessionProjection) -> Vec<String> {
    projection
        .messages
        .iter()
        .map(|message| match message.role {
            ProjectedRole::User => format!("You: {}", message.content),
            ProjectedRole::Assistant => format!("Agent: {}", message.content),
        })
        .collect()
}

#[cfg(test)]
#[path = "view_tests.rs"]
mod tests;
