use super::*;

#[test]
fn skill_id_creation_and_display() {
    let id = SkillId::new("code-review");
    assert_eq!(id.as_str(), "code-review");
    assert_eq!(id.to_string(), "code-review");
}

#[test]
fn skill_id_ordering() {
    let mut ids = [
        SkillId::new("zeta"),
        SkillId::new("alpha"),
        SkillId::new("beta"),
    ];
    ids.sort();
    assert_eq!(ids[0].as_str(), "alpha");
    assert_eq!(ids[1].as_str(), "beta");
    assert_eq!(ids[2].as_str(), "zeta");
}

#[test]
fn skill_id_serde_roundtrip() {
    let id = SkillId::new("code-review");
    let json = serde_json::to_string(&id).unwrap();
    let back: SkillId = serde_json::from_str(&json).unwrap();
    assert_eq!(back, id);
}

#[test]
fn skill_activation_defaults_to_manual() {
    let activation = SkillActivation::default();
    assert_eq!(activation.mode, SkillActivationMode::Manual);
    assert!(activation.keywords.is_empty());
}

#[test]
fn skill_source_kind_serde_is_snake_case() {
    assert_eq!(
        serde_json::to_value(SkillSourceKind::Builtin).unwrap(),
        serde_json::json!("builtin")
    );
    assert_eq!(
        serde_json::to_value(SkillSourceKind::User).unwrap(),
        serde_json::json!("user")
    );
    assert_eq!(
        serde_json::to_value(SkillSourceKind::Workspace).unwrap(),
        serde_json::json!("workspace")
    );
    assert_eq!(
        serde_json::to_value(SkillSourceKind::Plugin).unwrap(),
        serde_json::json!("plugin")
    );
}

#[test]
fn skill_metadata_serde_roundtrip() {
    let metadata = SkillMetadata {
        id: SkillId::new("test"),
        name: "Test Skill".into(),
        description: "A test".into(),
        version: Some("1.0.0".into()),
        source: SkillSource {
            kind: SkillSourceKind::User,
            root: "/tmp/skills".into(),
            path: "/tmp/skills/test/SKILL.md".into(),
        },
        activation: SkillActivation {
            mode: SkillActivationMode::Suggest,
            keywords: vec!["audit".into()],
        },
        permissions: SkillPermissionDeclaration {
            tools: vec!["shell".into()],
            can_request_tools: vec!["search".into()],
        },
    };
    let json = serde_json::to_string_pretty(&metadata).unwrap();
    let back: SkillMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(back, metadata);
}
