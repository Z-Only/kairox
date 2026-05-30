use super::*;

#[test]
fn skill_settings_view_distinguishes_scope_and_update_state() {
    let view = SkillSettingsView {
        settings_id: "project:review".to_string(),
        id: "review".to_string(),
        name: "review".to_string(),
        description: "Review code".to_string(),
        version: Some("1.2.3".to_string()),
        scope: SkillSettingsScope::Project,
        path: "/workspace/.kairox/skills/review/SKILL.md".to_string(),
        enabled: true,
        activation_mode: "suggest".to_string(),
        tools: vec!["fs.read".to_string()],
        can_request_tools: vec!["shell".to_string()],
        permission_summary: "tools: fs.read; can request: shell".to_string(),
        install_source: SkillInstallSource::Registry,
        update_state: SkillUpdateState::UpdateAvailable,
        effective: true,
        shadowed_by: None,
        valid: true,
        validation_error: None,
        editable: true,
        deletable: true,
    };

    assert_eq!(view.scope, SkillSettingsScope::Project);
    assert_eq!(view.update_state, SkillUpdateState::UpdateAvailable);
    assert!(view.editable);
}
