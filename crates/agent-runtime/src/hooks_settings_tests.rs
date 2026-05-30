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
fn upsert_hook_preserves_existing_config() {
    let path = write_config_fixture(
        "instructions = \"Be concise.\"\n\n[profiles.fast]\nprovider = \"fake\"\nmodel_id = \"fake\"\n",
    );
    let input = HookSettingsInput {
        scope: agent_core::ConfigScope::User,
        id: "verify".into(),
        event: "Stop".into(),
        matcher: Some("*".into()),
        command: "cargo test --workspace --all-targets".into(),
        status_message: Some("Running tests".into()),
        timeout_secs: Some(120),
        enabled: true,
    };

    upsert_hook(&input, &path).expect("upsert should succeed");

    let raw = std::fs::read_to_string(&path).expect("should read back");
    assert!(raw.contains("instructions = \"Be concise.\""));
    assert!(raw.contains("[profiles.fast]"));
    assert!(raw.contains("[features]"));
    assert!(raw.contains("hooks = true"));
    assert!(raw.contains("[hooks.Stop.verify]"));
    assert!(raw.contains("command = \"cargo test --workspace --all-targets\""));
}

#[test]
fn delete_hook_removes_empty_event_table() {
    let path = write_config_fixture(
        "[hooks.Stop.verify]\nmatcher = \"*\"\ncommand = \"cargo test\"\nenabled = true\n",
    );

    delete_hook(&path, "Stop", "verify").expect("delete should succeed");

    let raw = std::fs::read_to_string(&path).expect("should read back");
    assert!(!raw.contains("verify"));
    assert!(!raw.contains("Stop"));
    assert!(read_hooks_from_config(&path, agent_core::ConfigScope::User)
        .expect("read should succeed")
        .is_empty());
}

#[test]
fn builtin_templates_include_stop_validation() {
    let templates = builtin_hook_templates();
    let stop_validation = templates
        .iter()
        .find(|template| template.id == "stop-validation")
        .expect("stop validation template should exist");

    assert_eq!(stop_validation.event, "Stop");
    assert!(stop_validation.command.contains("cargo test"));
}
