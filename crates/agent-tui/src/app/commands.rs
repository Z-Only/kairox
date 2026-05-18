use agent_core::projection::{ProjectedMessage, ProjectedRole};
use agent_core::{ActivateSkillRequest, AppFacade, DeactivateSkillRequest};

use super::App;
use crate::components::Command;

pub async fn dispatch_commands<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
    commands: Vec<Command>,
) where
    F: AppFacade + ?Sized,
{
    for command in commands {
        match command {
            Command::ListSkills => match AppFacade::list_skills(runtime.as_ref()).await {
                Ok(skills) if skills.is_empty() => {
                    push_status_message(app, "No skills discovered".to_string());
                }
                Ok(skills) => {
                    let skill_lines = skills
                        .iter()
                        .map(|skill| format!("- {}: {}", skill.id, skill.description))
                        .collect::<Vec<_>>()
                        .join("\n");
                    push_status_message(app, format!("Available skills:\n{skill_lines}"));
                }
                Err(error) => {
                    push_status_message(app, format!("[skills error: {error}]"));
                }
            },
            Command::ShowSkill { skill_id } => {
                match AppFacade::get_skill(runtime.as_ref(), skill_id.clone()).await {
                    Ok(Some(skill)) => {
                        push_status_message(
                            app,
                            format!(
                                "Skill {}\n{}\n\n{}",
                                skill.view.id, skill.view.description, skill.body_markdown
                            ),
                        );
                    }
                    Ok(None) => {
                        push_status_message(app, format!("[skill not found: {skill_id}]"));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill show error: {error}]"));
                    }
                }
            }
            Command::ActivateSkill {
                workspace_id,
                session_id,
                skill_id,
            } => {
                let request = ActivateSkillRequest {
                    workspace_id,
                    session_id,
                    skill_id: skill_id.clone(),
                };
                match AppFacade::activate_skill(runtime.as_ref(), request).await {
                    Ok(active_skill) => {
                        push_status_message(app, format!("activated {}", active_skill.skill_id));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill activate error: {error}]"));
                    }
                }
            }
            Command::DeactivateSkill {
                workspace_id,
                session_id,
                skill_id,
            } => {
                let request = DeactivateSkillRequest {
                    workspace_id,
                    session_id,
                    skill_id: skill_id.clone(),
                };
                match AppFacade::deactivate_skill(runtime.as_ref(), request).await {
                    Ok(()) => {
                        push_status_message(app, format!("deactivated {skill_id}"));
                    }
                    Err(error) => {
                        push_status_message(app, format!("[skill deactivate error: {error}]"));
                    }
                }
            }
            _ => {}
        }
    }
}

fn push_status_message(app: &mut App, content: String) {
    app.state.current_session.messages.push(ProjectedMessage {
        role: ProjectedRole::Assistant,
        content,
    });
    app.state.render_scheduler.mark_dirty();
}
