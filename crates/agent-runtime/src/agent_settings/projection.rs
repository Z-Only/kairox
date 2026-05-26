use std::collections::HashMap;
use std::path::{Path, PathBuf};

use agent_core::facade::{AgentSettingsScope, AgentSettingsView};
use agent_core::{CoreError, Result};

use super::parser::{parse_agent_markdown, ParsedAgentMarkdown};
use super::{scope_label, settings_id, AgentSettingsRoots};

pub(super) async fn list_agent_settings(
    roots: &AgentSettingsRoots,
) -> Result<Vec<AgentSettingsView>> {
    let mut views = builtin_agent_views(roots);
    if let Some(root) = &roots.user_root {
        views.extend(discover_agent_files(root, AgentSettingsScope::User).await?);
    }
    if let Some(root) = &roots.workspace_root {
        views.extend(discover_agent_files(root, AgentSettingsScope::Project).await?);
    }
    mark_effective_agents(&mut views);
    views.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| right.scope.cmp(&left.scope))
    });
    Ok(views)
}

pub(super) async fn find_agent_settings_view(
    roots: &AgentSettingsRoots,
    agent_identifier: &str,
) -> Result<AgentSettingsView> {
    let views = list_agent_settings(roots).await?;
    let matching_settings_id_views = views
        .iter()
        .filter(|view| view.settings_id == agent_identifier)
        .cloned()
        .collect::<Vec<_>>();
    match matching_settings_id_views.as_slice() {
        [view] => return Ok(view.clone()),
        [] => {}
        _ => {
            return Err(CoreError::InvalidState(format!(
                "ambiguous agent settings id: {agent_identifier}"
            )));
        }
    }

    let matching_views = views
        .into_iter()
        .filter(|view| view.name == agent_identifier)
        .collect::<Vec<_>>();
    match matching_views.as_slice() {
        [view] => Ok(view.clone()),
        [] => Err(CoreError::InvalidState(format!(
            "agent not found: {agent_identifier}"
        ))),
        views => Err(CoreError::InvalidState(format!(
            "ambiguous agent name: {agent_identifier}; matching scopes: {}",
            views
                .iter()
                .map(|view| scope_label(view.scope))
                .collect::<Vec<_>>()
                .join(", ")
        ))),
    }
}

/// Find the effective (highest-precedence, enabled) agent for a given name.
pub(crate) fn effective_agent_by_name<'a>(
    views: &'a [AgentSettingsView],
    name: &str,
) -> Option<&'a AgentSettingsView> {
    views
        .iter()
        .find(|view| view.effective && view.valid && view.name == name)
}

async fn discover_agent_files(
    root: &Path,
    scope: AgentSettingsScope,
) -> Result<Vec<AgentSettingsView>> {
    let mut views = Vec::new();
    let mut entries = match tokio::fs::read_dir(root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(CoreError::InvalidState(format!(
                "failed to read agent dir: {error}"
            )));
        }
    };

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to read agent entry: {error}")))?
    {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("md") {
            continue;
        }
        let raw = tokio::fs::read_to_string(&path)
            .await
            .map_err(|error| CoreError::InvalidState(format!("failed to read agent: {error}")))?;
        match parse_agent_markdown(&raw) {
            Ok(agent) => views.push(view_from_parsed(scope, path, agent, true, None)),
            Err(error) => views.push(invalid_view(scope, path, error.to_string())),
        }
    }

    Ok(views)
}

fn builtin_agent_views(roots: &AgentSettingsRoots) -> Vec<AgentSettingsView> {
    let builtin_root = roots.builtin_root.clone();
    builtin_agents()
        .into_iter()
        .map(|agent| {
            let path = builtin_root
                .as_ref()
                .map(|root| root.join(format!("{}.md", agent.name)))
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| format!("builtin://{}", agent.name));
            AgentSettingsView {
                settings_id: settings_id(AgentSettingsScope::Builtin, &agent.name),
                name: agent.name,
                description: agent.description,
                scope: AgentSettingsScope::Builtin,
                path,
                tools: agent.tools,
                model_profile: agent.model_profile,
                permission_mode: agent.permission_mode,
                skills: agent.skills,
                nickname_candidates: agent.nickname_candidates,
                enabled: agent.enabled,
                instructions: agent.instructions,
                effective: false,
                shadowed_by: None,
                valid: true,
                validation_error: None,
                editable: false,
                deletable: false,
            }
        })
        .collect()
}

fn builtin_agents() -> Vec<ParsedAgentMarkdown> {
    vec![
        ParsedAgentMarkdown {
            name: "default".into(),
            description: "General-purpose fallback agent.".into(),
            tools: Vec::new(),
            model_profile: None,
            permission_mode: None,
            skills: Vec::new(),
            nickname_candidates: vec!["Default".into()],
            enabled: true,
            instructions: "Handle general tasks that do not need a more specific agent.".into(),
        },
        ParsedAgentMarkdown {
            name: "worker".into(),
            description: "Execution-focused agent for implementation and fixes.".into(),
            tools: Vec::new(),
            model_profile: None,
            permission_mode: Some("workspace_write".into()),
            skills: Vec::new(),
            nickname_candidates: vec!["Worker".into()],
            enabled: true,
            instructions:
                "Implement scoped changes, run focused validation, and report changed files."
                    .into(),
        },
        ParsedAgentMarkdown {
            name: "explorer".into(),
            description: "Read-heavy codebase exploration agent.".into(),
            tools: vec!["fs.read".into(), "search".into()],
            model_profile: None,
            permission_mode: Some("read_only".into()),
            skills: Vec::new(),
            nickname_candidates: vec!["Explorer".into()],
            enabled: true,
            instructions: "Map code paths, cite files and symbols, and avoid editing files.".into(),
        },
        ParsedAgentMarkdown {
            name: "code-reviewer".into(),
            description: "Review code for correctness, security, regressions, and missing tests."
                .into(),
            tools: vec!["fs.read".into(), "search".into(), "shell".into()],
            model_profile: None,
            permission_mode: Some("read_only".into()),
            skills: Vec::new(),
            nickname_candidates: vec!["Reviewer".into()],
            enabled: true,
            instructions: "Lead with concrete findings ordered by severity. Focus on bugs, regressions, security, and missing tests.".into(),
        },
        ParsedAgentMarkdown {
            name: "test-runner".into(),
            description: "Run focused tests, diagnose failures, and report validation evidence."
                .into(),
            tools: vec!["shell".into(), "fs.read".into(), "search".into()],
            model_profile: None,
            permission_mode: Some("workspace_write".into()),
            skills: Vec::new(),
            nickname_candidates: vec!["Test".into()],
            enabled: true,
            instructions: "Run the smallest useful test first, inspect failures, and preserve test intent when fixing issues.".into(),
        },
    ]
}

fn mark_effective_agents(views: &mut [AgentSettingsView]) {
    let mut highest_by_name: HashMap<String, AgentSettingsScope> = HashMap::new();
    for view in views.iter().filter(|view| view.valid && view.enabled) {
        highest_by_name
            .entry(view.name.clone())
            .and_modify(|scope| {
                if view.scope > *scope {
                    *scope = view.scope;
                }
            })
            .or_insert(view.scope);
    }

    for view in views {
        let Some(active_scope) = highest_by_name.get(&view.name) else {
            continue;
        };
        view.effective = view.valid && view.scope == *active_scope;
        view.shadowed_by = if view.effective {
            None
        } else {
            Some(scope_label(*active_scope).to_string())
        };
    }
}

fn view_from_parsed(
    scope: AgentSettingsScope,
    path: PathBuf,
    agent: ParsedAgentMarkdown,
    valid: bool,
    validation_error: Option<String>,
) -> AgentSettingsView {
    AgentSettingsView {
        settings_id: settings_id(scope, &agent.name),
        name: agent.name,
        description: agent.description,
        scope,
        path: path.display().to_string(),
        tools: agent.tools,
        model_profile: agent.model_profile,
        permission_mode: agent.permission_mode,
        skills: agent.skills,
        nickname_candidates: agent.nickname_candidates,
        enabled: agent.enabled,
        instructions: agent.instructions,
        effective: false,
        shadowed_by: None,
        valid,
        validation_error,
        editable: scope != AgentSettingsScope::Builtin,
        deletable: scope != AgentSettingsScope::Builtin,
    }
}

fn invalid_view(
    scope: AgentSettingsScope,
    path: PathBuf,
    validation_error: String,
) -> AgentSettingsView {
    let name = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("invalid-agent")
        .to_string();
    AgentSettingsView {
        settings_id: settings_id(scope, &name),
        name,
        description: String::new(),
        scope,
        path: path.display().to_string(),
        tools: Vec::new(),
        model_profile: None,
        permission_mode: None,
        skills: Vec::new(),
        nickname_candidates: Vec::new(),
        enabled: false,
        instructions: String::new(),
        effective: false,
        shadowed_by: None,
        valid: false,
        validation_error: Some(validation_error),
        editable: scope != AgentSettingsScope::Builtin,
        deletable: scope != AgentSettingsScope::Builtin,
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;

    fn view(
        name: &str,
        scope: AgentSettingsScope,
        valid: bool,
        enabled: bool,
    ) -> AgentSettingsView {
        AgentSettingsView {
            settings_id: settings_id(scope, name),
            name: name.to_string(),
            description: String::new(),
            scope,
            path: format!("/mock/{name}.md"),
            tools: Vec::new(),
            model_profile: None,
            permission_mode: None,
            skills: Vec::new(),
            nickname_candidates: Vec::new(),
            enabled,
            instructions: String::new(),
            effective: false,
            shadowed_by: None,
            valid,
            validation_error: None,
            editable: scope != AgentSettingsScope::Builtin,
            deletable: scope != AgentSettingsScope::Builtin,
        }
    }

    fn parsed(name: &str) -> ParsedAgentMarkdown {
        ParsedAgentMarkdown {
            name: name.to_string(),
            description: format!("{name} description"),
            tools: vec!["shell".into()],
            model_profile: Some("fast".into()),
            permission_mode: Some("ask".into()),
            skills: vec!["audit".into()],
            nickname_candidates: vec![name.into()],
            enabled: true,
            instructions: format!("Run {name}.\n"),
        }
    }

    #[test]
    fn effective_agent_by_name_returns_matching_effective_valid_view() {
        let mut hit = view("planner", AgentSettingsScope::User, true, true);
        hit.effective = true;
        let views = vec![hit.clone()];
        let found = effective_agent_by_name(&views, "planner").expect("should find");
        assert_eq!(found.settings_id, hit.settings_id);
    }

    #[test]
    fn effective_agent_by_name_skips_non_effective_views() {
        let mut shadowed = view("planner", AgentSettingsScope::User, true, true);
        shadowed.effective = false;
        shadowed.shadowed_by = Some("Project".into());
        let views = vec![shadowed];
        assert!(effective_agent_by_name(&views, "planner").is_none());
    }

    #[test]
    fn effective_agent_by_name_skips_invalid_views_even_if_marked_effective() {
        let mut invalid = view("planner", AgentSettingsScope::User, false, true);
        invalid.effective = true;
        let views = vec![invalid];
        assert!(effective_agent_by_name(&views, "planner").is_none());
    }

    #[test]
    fn effective_agent_by_name_returns_none_when_no_view_has_matching_name() {
        let mut other = view("worker", AgentSettingsScope::User, true, true);
        other.effective = true;
        let views = vec![other];
        assert!(effective_agent_by_name(&views, "planner").is_none());
    }

    #[test]
    fn mark_effective_agents_picks_highest_scope_when_multiple_enabled_and_valid() {
        let mut views = vec![
            view("planner", AgentSettingsScope::Builtin, true, true),
            view("planner", AgentSettingsScope::User, true, true),
            view("planner", AgentSettingsScope::Project, true, true),
        ];
        mark_effective_agents(&mut views);

        let project = views
            .iter()
            .find(|view| view.scope == AgentSettingsScope::Project)
            .unwrap();
        let user = views
            .iter()
            .find(|view| view.scope == AgentSettingsScope::User)
            .unwrap();
        let builtin = views
            .iter()
            .find(|view| view.scope == AgentSettingsScope::Builtin)
            .unwrap();

        assert!(project.effective, "highest-scope project should win");
        assert!(project.shadowed_by.is_none());
        assert!(!user.effective);
        assert_eq!(user.shadowed_by.as_deref(), Some("Project"));
        assert!(!builtin.effective);
        assert_eq!(builtin.shadowed_by.as_deref(), Some("Project"));
    }

    #[test]
    fn mark_effective_agents_does_not_treat_disabled_view_as_candidate() {
        let mut views = vec![
            view("planner", AgentSettingsScope::User, true, true),
            view("planner", AgentSettingsScope::Project, true, false),
        ];
        mark_effective_agents(&mut views);

        let user = views
            .iter()
            .find(|view| view.scope == AgentSettingsScope::User)
            .unwrap();
        assert!(
            user.effective,
            "user should win because project agent is disabled"
        );
        let project = views
            .iter()
            .find(|view| view.scope == AgentSettingsScope::Project)
            .unwrap();
        assert!(!project.effective);
    }

    #[test]
    fn mark_effective_agents_does_not_treat_invalid_view_as_candidate() {
        let mut views = vec![
            view("planner", AgentSettingsScope::User, true, true),
            view("planner", AgentSettingsScope::Project, false, true),
        ];
        mark_effective_agents(&mut views);

        let user = views
            .iter()
            .find(|view| view.scope == AgentSettingsScope::User)
            .unwrap();
        assert!(user.effective);
        let project = views
            .iter()
            .find(|view| view.scope == AgentSettingsScope::Project)
            .unwrap();
        assert!(!project.effective);
        // Invalid views still carry shadowed_by because the second pass
        // visits every view, regardless of validity, as long as some other
        // (valid+enabled) view with the same name claimed an active scope.
        assert_eq!(project.shadowed_by.as_deref(), Some("User"));
    }

    #[test]
    fn mark_effective_agents_handles_orthogonal_agents_independently() {
        let mut views = vec![
            view("planner", AgentSettingsScope::User, true, true),
            view("worker", AgentSettingsScope::Project, true, true),
        ];
        mark_effective_agents(&mut views);
        assert!(views.iter().all(|view| view.effective));
    }

    #[test]
    fn builtin_agent_views_uses_builtin_root_path_when_provided() {
        let roots = AgentSettingsRoots {
            workspace_root: None,
            user_root: None,
            builtin_root: Some(PathBuf::from("/mock/builtin")),
        };
        let views = builtin_agent_views(&roots);
        let default = views.iter().find(|v| v.name == "default").unwrap();
        assert_eq!(default.path, "/mock/builtin/default.md");
        assert_eq!(default.scope, AgentSettingsScope::Builtin);
        assert!(!default.editable);
        assert!(!default.deletable);
    }

    #[test]
    fn builtin_agent_views_uses_builtin_scheme_when_root_unset() {
        let roots = AgentSettingsRoots {
            workspace_root: None,
            user_root: None,
            builtin_root: None,
        };
        let views = builtin_agent_views(&roots);
        assert!(!views.is_empty(), "builtins should never be empty");
        for view in &views {
            assert!(
                view.path.starts_with("builtin://"),
                "expected builtin:// scheme, got {}",
                view.path
            );
        }
    }

    #[test]
    fn builtin_agents_returns_the_documented_built_in_set() {
        let names: Vec<String> = builtin_agents().into_iter().map(|a| a.name).collect();
        assert_eq!(
            names,
            vec![
                "default".to_string(),
                "worker".into(),
                "explorer".into(),
                "code-reviewer".into(),
                "test-runner".into(),
            ]
        );
    }

    #[test]
    fn view_from_parsed_marks_non_builtin_scope_as_editable_and_deletable() {
        let agent = parsed("planner");
        let view = view_from_parsed(
            AgentSettingsScope::User,
            "/x/planner.md".into(),
            agent,
            true,
            None,
        );
        assert_eq!(view.name, "planner");
        assert_eq!(view.path, "/x/planner.md");
        assert_eq!(view.tools, vec!["shell"]);
        assert_eq!(view.permission_mode.as_deref(), Some("ask"));
        assert!(view.valid);
        assert!(view.editable);
        assert!(view.deletable);
        assert!(view.validation_error.is_none());
        assert!(!view.effective);
        assert!(view.shadowed_by.is_none());
    }

    #[test]
    fn view_from_parsed_marks_builtin_scope_as_neither_editable_nor_deletable() {
        let agent = parsed("default");
        let view = view_from_parsed(
            AgentSettingsScope::Builtin,
            "/builtin/default.md".into(),
            agent,
            true,
            None,
        );
        assert!(!view.editable);
        assert!(!view.deletable);
    }

    #[test]
    fn invalid_view_uses_file_stem_for_name_and_carries_validation_error() {
        let view = invalid_view(
            AgentSettingsScope::Project,
            PathBuf::from("/repo/.kairox/agents/broken.md"),
            "missing required agent field: name".to_string(),
        );
        assert_eq!(view.name, "broken");
        assert!(!view.valid);
        assert_eq!(
            view.validation_error.as_deref(),
            Some("missing required agent field: name")
        );
        assert!(view.editable);
        assert!(view.deletable);
        assert_eq!(view.path, "/repo/.kairox/agents/broken.md");
        assert_eq!(view.scope, AgentSettingsScope::Project);
    }

    #[test]
    fn invalid_view_falls_back_to_placeholder_when_path_has_no_file_stem() {
        let view = invalid_view(
            AgentSettingsScope::User,
            PathBuf::from("/"),
            "broken".to_string(),
        );
        assert_eq!(view.name, "invalid-agent");
    }
}
