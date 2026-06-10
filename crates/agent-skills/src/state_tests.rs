use super::*;

#[tokio::test]
async fn state_file_persists_disabled_skill_without_touching_skill_markdown() {
    let root = tempfile::tempdir().expect("root should exist");
    let skill_directory = root.path().join("review");
    std::fs::create_dir_all(&skill_directory).expect("skill directory should exist");
    let skill_path = skill_directory.join("SKILL.md");
    std::fs::write(
        &skill_path,
        "---\nname: review\ndescription: Review code\n---\nBody\n",
    )
    .expect("skill should be written");

    let state_path = root.path().join("skills-state.toml");
    let mut state = SkillsStateFile::default();
    state.set_enabled("review", false);
    write_skills_state(&state_path, &state)
        .await
        .expect("state should write");

    let reloaded = read_skills_state(&state_path)
        .await
        .expect("state should read");
    assert_eq!(
        reloaded.skill("review").and_then(|entry| entry.enabled),
        Some(false)
    );
    let markdown = std::fs::read_to_string(skill_path).expect("skill markdown should remain");
    assert!(markdown.contains("description: Review code"));
}

#[tokio::test]
async fn read_skills_state_accepts_standard_toml_comments_and_escapes() {
    let root = tempfile::tempdir().expect("root should exist");
    let state_path = root.path().join("skills-state.toml");
    std::fs::write(
        &state_path,
        "[skills.\"review\"]\n\
         enabled = true # inline comments are valid TOML\n\
         activation_mode = \"suggest\"\n\
         remote = \"line\\nvalue with \\\"quotes\\\" and \\\\ slash\"\n",
    )
    .expect("state should be written");

    let reloaded = read_skills_state(&state_path)
        .await
        .expect("state should read standard TOML");
    let entry = reloaded.skill("review").expect("review state should exist");

    assert_eq!(entry.enabled, Some(true));
    assert_eq!(entry.activation_mode, Some(SkillActivationMode::Suggest));
    assert_eq!(
        entry.remote.as_deref(),
        Some("line\nvalue with \"quotes\" and \\ slash")
    );
}

#[tokio::test]
async fn write_skills_state_round_trips_all_fields() {
    let root = tempfile::tempdir().expect("root should exist");
    let state_path = root.path().join("nested").join("skills-state.toml");
    let mut state = SkillsStateFile::default();
    state.skills.insert(
        "review.skill".to_owned(),
        SkillStateEntry {
            enabled: Some(false),
            activation_mode: Some(SkillActivationMode::Auto),
            install_source: Some("git".to_owned()),
            remote: Some("line\nvalue with \"quotes\" and \\ slash".to_owned()),
            version: Some("1.2.3".to_owned()),
            last_update_check: Some("2026-05-10T13:25:00Z".to_owned()),
            update_available: Some(true),
        },
    );

    write_skills_state(&state_path, &state)
        .await
        .expect("state should write");
    let reloaded = read_skills_state(&state_path)
        .await
        .expect("state should read");

    assert_eq!(reloaded, state);
}

#[tokio::test]
async fn read_skills_state_returns_default_when_file_missing() {
    let root = tempfile::tempdir().expect("root should exist");
    let state_path = root.path().join("nonexistent.toml");

    let state = read_skills_state(&state_path)
        .await
        .expect("missing file should return default");
    assert!(state.skills.is_empty());
}

#[tokio::test]
async fn read_skills_state_rejects_invalid_toml() {
    let root = tempfile::tempdir().expect("root should exist");
    let state_path = root.path().join("skills-state.toml");
    std::fs::write(&state_path, "this is not {{{ valid toml").expect("write should succeed");

    let result = read_skills_state(&state_path).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn skill_mut_creates_entry_when_absent() {
    let mut state = SkillsStateFile::default();
    assert!(state.skill("new-skill").is_none());

    let entry = state.skill_mut("new-skill");
    entry.activation_mode = Some(SkillActivationMode::Auto);
    entry.install_source = Some("git".to_owned());

    assert_eq!(
        state.skill("new-skill").unwrap().activation_mode,
        Some(SkillActivationMode::Auto)
    );
    assert_eq!(
        state.skill("new-skill").unwrap().install_source.as_deref(),
        Some("git")
    );
}

#[tokio::test]
async fn set_enabled_overwrites_previous_value() {
    let mut state = SkillsStateFile::default();
    state.set_enabled("toggle", true);
    assert_eq!(state.skill("toggle").unwrap().enabled, Some(true));

    state.set_enabled("toggle", false);
    assert_eq!(state.skill("toggle").unwrap().enabled, Some(false));
}

#[tokio::test]
async fn write_skills_state_creates_parent_directories() {
    let root = tempfile::tempdir().expect("root should exist");
    let nested_path = root.path().join("a").join("b").join("c").join("state.toml");
    let mut state = SkillsStateFile::default();
    state.set_enabled("deep-skill", true);

    write_skills_state(&nested_path, &state)
        .await
        .expect("write to deeply nested path should succeed");

    let reloaded = read_skills_state(&nested_path)
        .await
        .expect("should read back");
    assert_eq!(
        reloaded.skill("deep-skill").and_then(|e| e.enabled),
        Some(true)
    );
}

#[tokio::test]
async fn read_skills_state_handles_empty_file() {
    let root = tempfile::tempdir().expect("root should exist");
    let state_path = root.path().join("skills-state.toml");
    std::fs::write(&state_path, "").expect("write empty file");

    let state = read_skills_state(&state_path)
        .await
        .expect("empty file should parse as default");
    assert!(state.skills.is_empty());
}

#[tokio::test]
async fn skill_returns_none_for_unknown_id() {
    let state = SkillsStateFile::default();
    assert!(state.skill("nonexistent").is_none());
}
