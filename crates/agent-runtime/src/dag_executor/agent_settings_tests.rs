use super::*;
use agent_core::facade::{AgentSettingsScope, AgentSettingsView};

fn make_view(name: &str, effective: bool, valid: bool) -> AgentSettingsView {
    AgentSettingsView {
        settings_id: format!("settings-{name}"),
        name: name.to_string(),
        description: format!("Test {name} agent"),
        scope: AgentSettingsScope::Project,
        path: format!("/agents/{name}.md"),
        tools: vec!["shell.exec".to_string()],
        model_profile: Some("gpt-4o".to_string()),
        reasoning_effort: Some("high".to_string()),
        skills: vec!["code-review".to_string()],
        nickname_candidates: vec![],
        enabled: true,
        instructions: format!("Custom instructions for {name}"),
        effective,
        shadowed_by: None,
        valid,
        validation_error: None,
        editable: true,
        deletable: true,
    }
}

#[test]
fn empty_views_returns_three_default_strategies() {
    let strategies = strategies_from_agent_settings(&[]);
    assert_eq!(strategies.len(), 3);
    assert!(strategies.contains_key(&AgentRole::Planner));
    assert!(strategies.contains_key(&AgentRole::Worker));
    assert!(strategies.contains_key(&AgentRole::Reviewer));
}

#[test]
fn default_strategies_have_correct_roles() {
    let strategies = strategies_from_agent_settings(&[]);
    assert_eq!(
        strategies.get(&AgentRole::Planner).unwrap().role(),
        AgentRole::Planner
    );
    assert_eq!(
        strategies.get(&AgentRole::Worker).unwrap().role(),
        AgentRole::Worker
    );
    assert_eq!(
        strategies.get(&AgentRole::Reviewer).unwrap().role(),
        AgentRole::Reviewer
    );
}

#[test]
fn matching_effective_valid_view_uses_custom_settings() {
    let views = vec![make_view("default", true, true)];
    let strategies = strategies_from_agent_settings(&views);

    let planner = strategies.get(&AgentRole::Planner).unwrap();
    assert_eq!(planner.role(), AgentRole::Planner);
    // Custom view provides model_profile override
    assert_eq!(planner.model_profile_override(), Some("gpt-4o"));
    assert_eq!(planner.reasoning_effort_override(), Some("high"));
    assert_eq!(planner.skills(), &["code-review"]);
    assert_eq!(planner.tools_allowlist(), &["shell.exec"]);
}

#[test]
fn non_effective_view_falls_back_to_default() {
    let views = vec![make_view("default", false, true)];
    let strategies = strategies_from_agent_settings(&views);

    let planner = strategies.get(&AgentRole::Planner).unwrap();
    // Default PlannerStrategy has no model_profile override
    assert_eq!(planner.model_profile_override(), None);
}

#[test]
fn invalid_view_falls_back_to_default() {
    let views = vec![make_view("default", true, false)];
    let strategies = strategies_from_agent_settings(&views);

    let planner = strategies.get(&AgentRole::Planner).unwrap();
    assert_eq!(planner.model_profile_override(), None);
}

#[test]
fn unrelated_name_does_not_match_any_role() {
    let views = vec![make_view("unrelated-agent", true, true)];
    let strategies = strategies_from_agent_settings(&views);

    // All three roles should use defaults
    assert_eq!(
        strategies
            .get(&AgentRole::Planner)
            .unwrap()
            .model_profile_override(),
        None
    );
    assert_eq!(
        strategies
            .get(&AgentRole::Worker)
            .unwrap()
            .model_profile_override(),
        None
    );
    assert_eq!(
        strategies
            .get(&AgentRole::Reviewer)
            .unwrap()
            .model_profile_override(),
        None
    );
}

#[test]
fn worker_view_applies_to_worker_role() {
    let views = vec![make_view("worker", true, true)];
    let strategies = strategies_from_agent_settings(&views);

    let worker = strategies.get(&AgentRole::Worker).unwrap();
    assert_eq!(worker.model_profile_override(), Some("gpt-4o"));
    assert_eq!(worker.skills(), &["code-review"]);

    // Planner and Reviewer should remain default
    assert_eq!(
        strategies
            .get(&AgentRole::Planner)
            .unwrap()
            .model_profile_override(),
        None
    );
    assert_eq!(
        strategies
            .get(&AgentRole::Reviewer)
            .unwrap()
            .model_profile_override(),
        None
    );
}

#[test]
fn reviewer_view_applies_to_reviewer_role() {
    let views = vec![make_view("code-reviewer", true, true)];
    let strategies = strategies_from_agent_settings(&views);

    let reviewer = strategies.get(&AgentRole::Reviewer).unwrap();
    assert_eq!(reviewer.model_profile_override(), Some("gpt-4o"));
    assert_eq!(reviewer.reasoning_effort_override(), Some("high"));

    // Other roles remain default
    assert_eq!(
        strategies
            .get(&AgentRole::Planner)
            .unwrap()
            .model_profile_override(),
        None
    );
}

#[test]
fn all_three_views_apply_simultaneously() {
    let views = vec![
        make_view("default", true, true),
        make_view("worker", true, true),
        make_view("code-reviewer", true, true),
    ];
    let strategies = strategies_from_agent_settings(&views);

    assert_eq!(
        strategies
            .get(&AgentRole::Planner)
            .unwrap()
            .model_profile_override(),
        Some("gpt-4o")
    );
    assert_eq!(
        strategies
            .get(&AgentRole::Worker)
            .unwrap()
            .model_profile_override(),
        Some("gpt-4o")
    );
    assert_eq!(
        strategies
            .get(&AgentRole::Reviewer)
            .unwrap()
            .model_profile_override(),
        Some("gpt-4o")
    );
}
