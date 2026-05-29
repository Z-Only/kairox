pub use super::command_types::*;

impl Command {
    pub fn destructive_confirmation_target(&self) -> Option<DestructiveConfirmationTarget> {
        match self {
            Self::ArchiveSession { session_id } => Some(DestructiveConfirmationTarget::new(
                "session.archive",
                session_id.to_string(),
                format!("archive session {session_id}"),
            )),
            Self::DeleteSession { session_id } => Some(DestructiveConfirmationTarget::new(
                "session.delete",
                session_id.to_string(),
                format!("permanently delete archived session {session_id}"),
            )),
            Self::RemoveProject { project_id } => Some(DestructiveConfirmationTarget::new(
                "project.remove",
                project_id.to_string(),
                format!("remove project {project_id}"),
            )),
            Self::DeleteMcpServerSettings { server_id } => {
                Some(DestructiveConfirmationTarget::new(
                    "mcp.settings.delete",
                    server_id.clone(),
                    format!("delete MCP server settings {server_id}"),
                ))
            }
            Self::UninstallMcpServer { server_id } => Some(DestructiveConfirmationTarget::new(
                "mcp.uninstall",
                server_id.clone(),
                format!("uninstall MCP server {server_id}"),
            )),
            Self::RemoveMcpCatalogSource { source_id } => Some(DestructiveConfirmationTarget::new(
                "mcp.source.remove",
                source_id.clone(),
                format!("remove MCP catalog source {source_id}"),
            )),
            Self::DeleteProfileSettings { alias } => Some(DestructiveConfirmationTarget::new(
                "model.profile.delete",
                alias.clone(),
                format!("delete model profile {alias}"),
            )),
            Self::DeleteHookSettings { event, id, .. } => Some(DestructiveConfirmationTarget::new(
                "hook.delete",
                format!("{event}:{id}"),
                format!("delete hook {event}/{id}"),
            )),
            Self::DeleteAgentSettings { settings_id } => Some(DestructiveConfirmationTarget::new(
                "agent.delete",
                settings_id.clone(),
                format!("delete agent profile {settings_id}"),
            )),
            Self::DeleteSkillSettings { skill_id } => Some(DestructiveConfirmationTarget::new(
                "skill.delete",
                skill_id.clone(),
                format!("delete skill {skill_id}"),
            )),
            Self::RemoveSkillSource { source_id } => Some(DestructiveConfirmationTarget::new(
                "skill.source.remove",
                source_id.clone(),
                format!("remove skill source {source_id}"),
            )),
            Self::DeletePluginSettings { settings_id } => Some(DestructiveConfirmationTarget::new(
                "plugin.delete",
                settings_id.clone(),
                format!("delete plugin {settings_id}"),
            )),
            _ => None,
        }
    }
}
