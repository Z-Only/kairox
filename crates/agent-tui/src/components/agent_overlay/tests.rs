use agent_core::facade::{AgentSettingsInput, AgentSettingsScope, AgentSettingsView};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

use super::AgentOverlay;
use crate::components::{AgentOverlaySnapshot, Command};

fn agent(name: &str, scope: AgentSettingsScope) -> AgentSettingsView {
    let scope_label = match scope {
        AgentSettingsScope::Builtin => "Builtin",
        AgentSettingsScope::User => "User",
        AgentSettingsScope::Project => "Project",
    };
    AgentSettingsView {
        settings_id: format!("{scope_label}:{name}"),
        name: name.to_string(),
        description: format!("{name} description"),
        scope,
        path: format!("{name}.md"),
        tools: vec!["fs.read".to_string()],
        model_profile: Some("fast".to_string()),
        permission_mode: Some("read_only".to_string()),
        skills: vec!["kairox-dev-workflow".to_string()],
        nickname_candidates: vec![name.to_string()],
        enabled: true,
        instructions: format!("{name} instructions"),
        effective: scope != AgentSettingsScope::Builtin,
        shadowed_by: None,
        valid: true,
        validation_error: None,
        editable: scope != AgentSettingsScope::Builtin,
        deletable: scope != AgentSettingsScope::Builtin,
    }
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn show_lists_builtin_user_and_project_profiles() {
    let mut overlay = AgentOverlay::new();
    overlay.show(AgentOverlaySnapshot {
        agents: vec![
            agent("worker", AgentSettingsScope::Builtin),
            agent("worker", AgentSettingsScope::User),
            agent("reviewer", AgentSettingsScope::Project),
        ],
    });

    assert!(overlay.is_visible());
    assert_eq!(overlay.agents().len(), 3);
    assert_eq!(overlay.agents()[0].scope, AgentSettingsScope::Builtin);
    assert_eq!(overlay.agents()[1].scope, AgentSettingsScope::User);
    assert_eq!(overlay.agents()[2].scope, AgentSettingsScope::Project);
    assert_eq!(overlay.selected_index(), Some(0));
}

#[test]
fn copy_builtin_to_user_dispatches_command() {
    let mut overlay = AgentOverlay::new();
    overlay.show(AgentOverlaySnapshot {
        agents: vec![agent("worker", AgentSettingsScope::Builtin)],
    });

    let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Char('c')));

    assert!(matches!(
        &commands[..],
        [Command::CopyAgentSettings {
            settings_id,
            scope: AgentSettingsScope::User,
        }] if settings_id == "Builtin:worker"
    ));
}

#[test]
fn save_editor_dispatches_agent_settings_input() {
    let mut overlay = AgentOverlay::new();
    overlay.start_create_for_test(AgentSettingsScope::Project);
    overlay.replace_draft_for_test(AgentSettingsInput {
        scope: AgentSettingsScope::Project,
        name: "planner".to_string(),
        description: "Plans work".to_string(),
        tools: vec!["search".to_string()],
        model_profile: Some("reasoning".to_string()),
        permission_mode: Some("workspace_write".to_string()),
        skills: vec!["kairox-dev-workflow".to_string()],
        nickname_candidates: vec!["Planner".to_string()],
        enabled: false,
        instructions: "Break work into reviewable steps.".to_string(),
    });

    let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Enter));

    assert!(matches!(
        &commands[..],
        [Command::SaveAgentSettings { input }] if input.name == "planner"
            && input.scope == AgentSettingsScope::Project
            && input.tools == ["search"]
            && !input.enabled
    ));
}

#[test]
fn delete_editable_profile_dispatches_command() {
    let mut overlay = AgentOverlay::new();
    overlay.show(AgentOverlaySnapshot {
        agents: vec![agent("reviewer", AgentSettingsScope::User)],
    });

    let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Char('x')));

    assert!(matches!(
        &commands[..],
        [Command::DeleteAgentSettings { settings_id }] if settings_id == "User:reviewer"
    ));
}

#[test]
fn open_agents_dir_dispatches_command() {
    let mut overlay = AgentOverlay::new();
    overlay.show(AgentOverlaySnapshot { agents: Vec::new() });

    let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Char('o')));

    assert!(matches!(&commands[..], [Command::OpenAgentsDir]));
}
