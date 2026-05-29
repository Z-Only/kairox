use super::*;

fn view(name: &str, scope: AgentSettingsScope, valid: bool, enabled: bool) -> AgentSettingsView {
    AgentSettingsView {
        settings_id: settings_id(scope, name),
        name: name.to_string(),
        description: String::new(),
        scope,
        path: format!("/mock/{name}.md"),
        tools: Vec::new(),
        model_profile: None,
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
