use agent_config::Config;
use agent_core::facade::{SkillInstallSource, SkillSettingsScope, SkillsFacade};
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};

use super::{build_ui_runtime_from_store, default_data_dir, UiRuntimeOptions};

#[tokio::test]
async fn build_ui_runtime_discovers_builtin_skill_creator() {
    let home_dir = tempfile::tempdir().expect("home dir");
    let data_dir = default_data_dir(home_dir.path());
    let workspace_root = tempfile::tempdir().expect("workspace root");
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory store");
    let mut options = UiRuntimeOptions::new(
        home_dir.path().to_path_buf(),
        data_dir.clone(),
        "kairox.sqlite",
        workspace_root.path().to_path_buf(),
        ApprovalPolicy::default(),
        SandboxPolicy::default(),
        Config::defaults(),
        Vec::new(),
    );
    options.enable_marketplace = false;
    options.enable_mcp_servers = false;
    options.enable_lsp_servers = false;
    options.enable_plugin_skill_roots = false;

    let bootstrap = build_ui_runtime_from_store(store, options)
        .await
        .expect("runtime should build");

    let skills = SkillsFacade::list_skills(&bootstrap.runtime)
        .await
        .expect("skills should list");
    let skill = skills
        .iter()
        .find(|skill| skill.id == "skill-creator")
        .expect("builtin skill creator should be listed");
    assert_eq!(skill.source, "builtin");
    assert_eq!(skill.name, "skill-creator");

    let detail = SkillsFacade::get_skill(&bootstrap.runtime, "skill-creator".into())
        .await
        .expect("skill detail should load")
        .expect("skill creator detail");
    assert!(
        detail.body_markdown.contains("SKILL.md"),
        "skill creator should teach the SKILL.md format"
    );

    let settings = SkillsFacade::list_skill_settings(&bootstrap.runtime)
        .await
        .expect("skill settings should list");
    let settings_view = settings
        .iter()
        .find(|skill| skill.settings_id == "builtin:skill-creator")
        .expect("builtin settings view should include skill creator");
    assert_eq!(settings_view.scope, SkillSettingsScope::Builtin);
    assert_eq!(settings_view.install_source, SkillInstallSource::Builtin);
    assert!(!settings_view.editable);
    assert!(!settings_view.deletable);
    assert!(data_dir
        .join("builtin-skills/skill-creator/SKILL.md")
        .exists());
}
