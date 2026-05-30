use super::*;

fn write_config_fixture(raw: &str) -> std::path::PathBuf {
    let file = tempfile::NamedTempFile::new().expect("temp file created");
    let (_file, path) = file.keep().expect("temp file path kept");
    if !raw.is_empty() {
        std::fs::write(&path, raw).expect("fixture written");
    }
    path
}

#[test]
fn reads_instructions_from_config() {
    let path = write_config_fixture(
        "instructions = \"Be concise.\"\n\n[profiles.fast]\nprovider = \"openai_compatible\"\nmodel_id = \"gpt-4.1-mini\"\n",
    );
    let result = read_instructions(&path).expect("read should succeed");
    assert_eq!(result.as_deref(), Some("Be concise."));
}

#[test]
fn returns_none_when_key_absent() {
    let path = write_config_fixture(
        "[profiles.fast]\nprovider = \"openai_compatible\"\nmodel_id = \"gpt-4.1-mini\"\n",
    );
    let result = read_instructions(&path).expect("read should succeed");
    assert_eq!(result, None);
}

#[test]
fn returns_none_when_file_missing() {
    let result = read_instructions(Path::new("/nonexistent/instructions_test.toml"));
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
}

#[test]
fn writes_instructions_to_config() {
    let path = write_config_fixture(
        "[profiles.fast]\nprovider = \"openai_compatible\"\nmodel_id = \"gpt-4.1-mini\"\n",
    );
    write_instructions(&path, "Use Chinese.").expect("write should succeed");
    let raw = std::fs::read_to_string(&path).expect("should read back");
    assert!(raw.contains("instructions = \"Use Chinese.\""));
    // Existing content preserved
    assert!(raw.contains("[profiles.fast]"));
    assert!(raw.contains("provider = \"openai_compatible\""));
}

#[test]
fn removes_instructions_with_empty_text() {
    let path = write_config_fixture(
        "instructions = \"old value\"\n[profiles.fast]\nprovider = \"fake\"\nmodel_id = \"fake\"\n",
    );
    write_instructions(&path, "").expect("write should succeed");
    let raw = std::fs::read_to_string(&path).expect("should read back");
    assert!(!raw.contains("instructions"));
    assert!(raw.contains("[profiles.fast]"));
}

#[test]
fn creates_new_file_with_instructions() {
    let path = write_config_fixture("");
    write_instructions(&path, "New instructions.").expect("write should succeed");
    let raw = std::fs::read_to_string(&path).expect("should read back");
    assert!(raw.contains("instructions = \"New instructions.\""));
}

#[test]
fn get_system_prompt_returns_constant() {
    let prompt = get_system_prompt();
    assert!(prompt.contains("Kairox"));
    assert!(prompt.contains("Memory Protocol"));
}

#[test]
fn build_view_concatenates_layers() {
    let view = build_instructions_view(
        Some("User instructions.".into()),
        Some("Project instructions.".into()),
    );
    assert_eq!(view.user.as_deref(), Some("User instructions."));
    assert_eq!(view.project.as_deref(), Some("Project instructions."));
    assert!(view.system.contains("Kairox"));
}

#[test]
fn upsert_user_scope_writes_to_user_config() {
    let path = write_config_fixture("");
    let input = InstructionsUpdateInput {
        scope: agent_core::ConfigScope::User,
        text: "User level instructions.".into(),
    };
    upsert_instructions(&input, &path, None).expect("upsert should succeed");
    let raw = std::fs::read_to_string(&path).expect("should read back");
    assert!(raw.contains("instructions = \"User level instructions.\""));
}

#[test]
fn upsert_project_scope_requires_project_path() {
    let path = write_config_fixture("");
    let input = InstructionsUpdateInput {
        scope: agent_core::ConfigScope::Project,
        text: "Project instructions.".into(),
    };
    let result = upsert_instructions(&input, &path, Some(&path));
    assert!(result.is_ok());
}

#[test]
fn upsert_rejects_builtin_scope() {
    let path = write_config_fixture("");
    let input = InstructionsUpdateInput {
        scope: agent_core::ConfigScope::Builtin,
        text: "Should fail.".into(),
    };
    let result = upsert_instructions(&input, &path, None);
    assert!(result.is_err());
}
