use super::*;

pub(super) async fn dispatch<F>(runtime: &std::sync::Arc<F>, app: &mut App, command: Command)
where
    F: AppFacade + ?Sized,
{
    match command {
        Command::OpenSkillsOverlay => {
            refresh_skills_overlay(runtime, app, None, None).await;
            app.state.render_scheduler.mark_dirty_immediate();
        }
        Command::ListSkills if app.skills_overlay.is_visible() => {
            refresh_skills_overlay_with_current_query(runtime, app).await;
        }
        Command::ListSkills => match AppFacade::list_skills(runtime.as_ref()).await {
            Ok(skills) if skills.is_empty() => {
                common::push_status_message(app, "No skills discovered".to_string());
            }
            Ok(skills) => {
                let skill_lines = skills
                    .iter()
                    .map(|skill| format!("- {}: {}", skill.id, skill.description))
                    .collect::<Vec<_>>()
                    .join("\n");
                common::push_status_message(app, format!("Available skills:\n{skill_lines}"));
            }
            Err(error) => {
                common::push_status_message(app, format!("[skills error: {error}]"));
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
                        common::push_status_message(
                            app,
                            format!(
                                "Skill {}\n{}\n\n{}",
                                skill.view.id, skill.view.description, skill.body_markdown
                            ),
                        );
                    }
                }
                Ok(None) => {
                    common::push_status_message(app, format!("[skill not found: {skill_id}]"));
                }
                Err(error) => {
                    common::push_status_message(app, format!("[skill show error: {error}]"));
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
                        refresh_skills_overlay_with_current_query(runtime, app).await;
                    } else {
                        common::push_status_message(
                            app,
                            format!("activated {}", active_skill.skill_id),
                        );
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[skill activate error: {error}]"));
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
                        refresh_skills_overlay_with_current_query(runtime, app).await;
                    } else {
                        common::push_status_message(app, format!("deactivated {skill_id}"));
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[skill deactivate error: {error}]"));
                }
            }
        }
        Command::ListSkillCatalog { keyword, sources } if app.skills_overlay.is_visible() => {
            refresh_skills_overlay(runtime, app, keyword, sources).await;
        }
        Command::ListSkillCatalog { keyword, sources } => {
            let query = SkillCatalogQuery {
                keyword: keyword.clone(),
                sources: sources.clone(),
                limit: Some(50),
            };
            match AppFacade::list_skill_catalog(runtime.as_ref(), query).await {
                Ok(entries) if entries.is_empty() => {
                    let suffix = keyword
                        .as_deref()
                        .map(|value| format!(" for {value}"))
                        .unwrap_or_default();
                    common::push_status_message(app, format!("No catalog skills found{suffix}"));
                }
                Ok(entries) => {
                    let skill_lines = entries
                        .iter()
                        .map(|entry| {
                            format!("- {}: {} [{}]", entry.name, entry.description, entry.source)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    common::push_status_message(app, format!("Catalog skills:\n{skill_lines}"));
                }
                Err(error) => {
                    common::push_status_message(app, format!("[skill catalog error: {error}]"));
                }
            }
        }
        Command::InstallRemoteSkill { request } => {
            match AppFacade::install_remote_skill(runtime.as_ref(), request.clone()).await {
                Ok(skill) => {
                    if app.skills_overlay.is_visible() {
                        refresh_skills_overlay_with_current_query(runtime, app).await;
                    } else {
                        common::push_status_message(app, format!("installed skill {}", skill.id));
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[skill install error: {error}]"));
                }
            }
        }
        Command::InstallGithubSkill { request } => {
            match AppFacade::install_github_skill(runtime.as_ref(), request.clone()).await {
                Ok(skill) => {
                    if app.skills_overlay.is_visible() {
                        refresh_skills_overlay_with_current_query(runtime, app).await;
                    } else {
                        common::push_status_message(app, format!("installed skill {}", skill.id));
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[skill install error: {error}]"));
                }
            }
        }
        Command::UpdateSkillSettings { skill_id } => {
            match AppFacade::update_skill(runtime.as_ref(), skill_id.clone()).await {
                Ok(skill) => {
                    if app.skills_overlay.is_visible() {
                        refresh_skills_overlay_with_current_query(runtime, app).await;
                    } else {
                        common::push_status_message(app, format!("updated skill {}", skill.id));
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[skill update error: {error}]"));
                }
            }
        }
        Command::DeleteSkillSettings { skill_id } => {
            match AppFacade::delete_skill_settings(runtime.as_ref(), skill_id.clone()).await {
                Ok(()) => {
                    if app.skills_overlay.is_visible() {
                        refresh_skills_overlay_with_current_query(runtime, app).await;
                    } else {
                        common::push_status_message(app, format!("deleted skill {skill_id}"));
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[skill delete error: {error}]"));
                }
            }
        }
        Command::SetSkillEnabled { skill_id, enabled } => {
            match AppFacade::set_skill_enabled(runtime.as_ref(), skill_id.clone(), enabled).await {
                Ok(()) => {
                    if app.skills_overlay.is_visible() {
                        refresh_skills_overlay_with_current_query(runtime, app).await;
                    } else {
                        let state = if enabled { "enabled" } else { "disabled" };
                        common::push_status_message(app, format!("{state} skill {skill_id}"));
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[skill enable error: {error}]"));
                }
            }
        }
        Command::SetSkillSourceEnabled { source_id, enabled } => {
            match AppFacade::set_skill_source_enabled(runtime.as_ref(), source_id.clone(), enabled)
                .await
            {
                Ok(()) => refresh_skills_overlay_with_current_query(runtime, app).await,
                Err(error) => {
                    common::push_status_message(app, format!("[skill source error: {error}]"));
                }
            }
        }
        Command::AddSkillSource { config } => {
            match AppFacade::add_skill_source(runtime.as_ref(), config.clone()).await {
                Ok(()) => {
                    common::push_status_message(app, format!("added skill source {}", config.id));
                    refresh_skills_overlay_with_current_query(runtime, app).await;
                }
                Err(error) => {
                    common::push_status_message(app, format!("[skill source add error: {error}]"));
                }
            }
        }
        Command::RemoveSkillSource { source_id } => {
            match AppFacade::remove_skill_source(runtime.as_ref(), source_id.clone()).await {
                Ok(()) => {
                    common::push_status_message(app, format!("removed skill source {source_id}"));
                    refresh_skills_overlay_with_current_query(runtime, app).await;
                }
                Err(error) => {
                    common::push_status_message(
                        app,
                        format!("[skill source remove error: {error}]"),
                    );
                }
            }
        }
        Command::SearchRemoteSkills { query } => {
            match AppFacade::search_remote_skills(runtime.as_ref(), query.clone()).await {
                Ok(results) => {
                    if app.skills_overlay.is_visible() {
                        app.dispatch_effects(vec![CrossPanelEffect::SkillRemoteSearchResults(
                            results,
                        )]);
                        app.state.render_scheduler.mark_dirty();
                    } else {
                        if results.is_empty() {
                            common::push_status_message(
                                app,
                                format!("No remote skills found for \"{query}\""),
                            );
                        } else {
                            let lines = results
                                .iter()
                                .map(|r| format!("- {}: {}", r.name, r.description))
                                .collect::<Vec<_>>()
                                .join("\n");
                            common::push_status_message(
                                app,
                                format!("Remote skills for \"{query}\":\n{lines}"),
                            );
                        }
                    }
                }
                Err(error) => {
                    common::push_status_message(app, format!("[skill search error: {error}]"));
                }
            }
        }
        Command::RefreshSkillCatalog { keyword, sources } => {
            match AppFacade::refresh_skill_catalog(runtime.as_ref()).await {
                Ok(()) => {
                    if app.skills_overlay.is_visible() {
                        let (catalog_keyword, catalog_sources) =
                            if keyword.is_none() && sources.is_none() {
                                app.skills_overlay.catalog_query()
                            } else {
                                (keyword, sources)
                            };
                        refresh_skills_overlay(runtime, app, catalog_keyword, catalog_sources)
                            .await;
                    } else {
                        common::push_status_message(app, "refreshed skill catalog".to_string());
                    }
                }
                Err(error) => {
                    common::push_status_message(
                        app,
                        format!("[skill catalog refresh error: {error}]"),
                    );
                }
            }
        }
        _ => {}
    }
}

pub(super) async fn load_skill_entries<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
) -> Option<Vec<SkillEntry>>
where
    F: AppFacade + ?Sized,
{
    let skills = match AppFacade::list_skills(runtime.as_ref()).await {
        Ok(skills) => skills,
        Err(error) => {
            common::push_status_message(app, format!("[skills error: {error}]"));
            return None;
        }
    };

    let active_ids: std::collections::HashSet<String> =
        if let Some(session_id) = app.current_session_id.clone() {
            match AppFacade::list_active_skills(runtime.as_ref(), session_id).await {
                Ok(list) => list.into_iter().map(|a| a.skill_id).collect(),
                Err(error) => {
                    common::push_status_message(app, format!("[active skills error: {error}]"));
                    std::collections::HashSet::new()
                }
            }
        } else {
            std::collections::HashSet::new()
        };

    Some(
        skills
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
            .collect(),
    )
}

async fn refresh_skills_overlay<F>(
    runtime: &std::sync::Arc<F>,
    app: &mut App,
    catalog_keyword: Option<String>,
    catalog_sources: Option<Vec<String>>,
) where
    F: AppFacade + ?Sized,
{
    let Some(entries) = load_skill_entries(runtime, app).await else {
        return;
    };

    let installed = match AppFacade::list_skill_settings(runtime.as_ref()).await {
        Ok(installed) => installed,
        Err(error) => {
            common::push_status_message(app, format!("[skill settings error: {error}]"));
            Vec::new()
        }
    };

    let catalog = match AppFacade::list_skill_catalog(
        runtime.as_ref(),
        SkillCatalogQuery {
            keyword: catalog_keyword,
            sources: catalog_sources,
            limit: Some(50),
        },
    )
    .await
    {
        Ok(catalog) => catalog,
        Err(error) => {
            common::push_status_message(app, format!("[skill catalog error: {error}]"));
            Vec::new()
        }
    };

    let sources = match AppFacade::list_skill_sources(runtime.as_ref()).await {
        Ok(sources) => sources,
        Err(error) => {
            common::push_status_message(app, format!("[skill sources error: {error}]"));
            Vec::new()
        }
    };

    app.dispatch_effects(vec![CrossPanelEffect::ShowSkillsOverlay(
        SkillOverlaySnapshot {
            discovered: entries,
            installed,
            catalog,
            sources,
            install_target: SkillInstallTarget::User,
        },
    )]);
}

async fn refresh_skills_overlay_with_current_query<F>(runtime: &std::sync::Arc<F>, app: &mut App)
where
    F: AppFacade + ?Sized,
{
    let (catalog_keyword, catalog_sources) = app.skills_overlay.catalog_query();
    refresh_skills_overlay(runtime, app, catalog_keyword, catalog_sources).await;
}
