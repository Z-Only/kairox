use agent_core::projection::{ProjectedMessage, ProjectedRole};
use agent_core::{ActivateSkillRequest, AppFacade, DeactivateSkillRequest};

use super::App;
use crate::components::{Command, CrossPanelEffect, SkillEntry};

pub async fn dispatch_commands<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
    commands: Vec<Command>,
) where
    F: AppFacade + ?Sized,
{
    for command in commands {
        match command {
            Command::OpenSkillsOverlay => {
                refresh_skills_overlay(runtime, app).await;
                app.state.render_scheduler.mark_dirty_immediate();
            }
            Command::ListSkills if app.skills_overlay.is_visible() => {
                refresh_skills_overlay(runtime, app).await;
            }
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
                        if app.skills_overlay.is_visible() {
                            app.dispatch_effects(vec![CrossPanelEffect::ShowSkillBody {
                                skill_id: skill.view.id.clone(),
                                body: skill.body_markdown.clone(),
                            }]);
                            app.state.render_scheduler.mark_dirty();
                        } else {
                            push_status_message(
                                app,
                                format!(
                                    "Skill {}\n{}\n\n{}",
                                    skill.view.id, skill.view.description, skill.body_markdown
                                ),
                            );
                        }
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
                        if app.skills_overlay.is_visible() {
                            refresh_skills_overlay(runtime, app).await;
                        } else {
                            push_status_message(
                                app,
                                format!("activated {}", active_skill.skill_id),
                            );
                        }
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
                        if app.skills_overlay.is_visible() {
                            refresh_skills_overlay(runtime, app).await;
                        } else {
                            push_status_message(app, format!("deactivated {skill_id}"));
                        }
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

async fn refresh_skills_overlay<F>(runtime: &std::sync::Arc<F>, app: &mut App)
where
    F: AppFacade + ?Sized,
{
    let skills = match AppFacade::list_skills(runtime.as_ref()).await {
        Ok(skills) => skills,
        Err(error) => {
            push_status_message(app, format!("[skills error: {error}]"));
            return;
        }
    };

    let active_ids: std::collections::HashSet<String> =
        if let Some(session_id) = app.current_session_id.clone() {
            match AppFacade::list_active_skills(runtime.as_ref(), session_id).await {
                Ok(list) => list.into_iter().map(|a| a.skill_id).collect(),
                Err(error) => {
                    push_status_message(app, format!("[active skills error: {error}]"));
                    std::collections::HashSet::new()
                }
            }
        } else {
            std::collections::HashSet::new()
        };

    let entries: Vec<SkillEntry> = skills
        .into_iter()
        .map(|s| {
            let active = active_ids.contains(&s.id);
            SkillEntry {
                id: s.id,
                name: s.name,
                description: s.description,
                source: s.source,
                activation_mode: s.activation_mode,
                active,
            }
        })
        .collect();

    app.dispatch_effects(vec![CrossPanelEffect::ShowSkillsOverlay(entries)]);
}
