use agent_config::{Config, KnowledgeBaseConfig, KnowledgeBaseKind};
use agent_core::facade::{SkillInstallSource, SkillSettingsScope, SkillsFacade};
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};

use super::{
    build_knowledge_base_retrievers, build_ui_runtime_from_store, default_data_dir,
    UiRuntimeOptions,
};

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

#[tokio::test]
async fn build_ui_runtime_wires_configured_sqlite_fts_knowledge_base() {
    let home_dir = tempfile::tempdir().expect("home dir");
    let data_dir = default_data_dir(home_dir.path());
    let workspace_root = tempfile::tempdir().expect("workspace root");
    let kb_path = workspace_root.path().join("company.sqlite");
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory store");
    let mut config = Config::defaults();
    config.knowledge_bases = vec![(
        "company-docs".into(),
        KnowledgeBaseConfig {
            kind: KnowledgeBaseKind::SqliteFts,
            path: Some(kb_path.to_string_lossy().to_string()),
            table: Some("company_docs".into()),
            profile_aliases: vec!["fake".into()],
            ..KnowledgeBaseConfig::default()
        },
    )];
    let mut options = UiRuntimeOptions::new(
        home_dir.path().to_path_buf(),
        data_dir,
        "kairox.sqlite",
        workspace_root.path().to_path_buf(),
        ApprovalPolicy::default(),
        SandboxPolicy::default(),
        config,
        Vec::new(),
    );
    options.enable_marketplace = false;
    options.enable_mcp_servers = false;
    options.enable_lsp_servers = false;
    options.enable_plugin_skill_roots = false;

    let bootstrap = build_ui_runtime_from_store(store, options)
        .await
        .expect("runtime should build");

    assert!(bootstrap
        .runtime
        .knowledge_base_retrievers_snapshot()
        .contains_key("company-docs"));
    assert!(kb_path.exists());
}

#[tokio::test]
async fn build_knowledge_base_retrievers_rejects_sqlite_fts_without_path() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    let mut config = Config::defaults();
    config.knowledge_bases = vec![(
        "missing-path".into(),
        KnowledgeBaseConfig {
            kind: KnowledgeBaseKind::SqliteFts,
            ..KnowledgeBaseConfig::default()
        },
    )];

    let result = build_knowledge_base_retrievers(&config, workspace_root.path()).await;
    let Err(error) = result else {
        panic!("sqlite_fts knowledge bases require an explicit path");
    };

    assert!(
        error
            .to_string()
            .contains("knowledge base 'missing-path' missing SQLite path"),
        "{error}"
    );
}

#[tokio::test]
async fn build_knowledge_base_retrievers_skips_disabled_and_unwired_sources() {
    let workspace_root = tempfile::tempdir().expect("workspace root");
    let disabled_path = workspace_root.path().join("disabled.sqlite");
    let mut config = Config::defaults();
    config.knowledge_bases = vec![
        (
            "disabled-sqlite".into(),
            KnowledgeBaseConfig {
                kind: KnowledgeBaseKind::SqliteFts,
                enabled: false,
                path: Some(disabled_path.to_string_lossy().to_string()),
                ..KnowledgeBaseConfig::default()
            },
        ),
        (
            "remote-pinecone".into(),
            KnowledgeBaseConfig {
                kind: KnowledgeBaseKind::Pinecone,
                endpoint: Some("https://pinecone.example.com".into()),
                index_name: Some("support".into()),
                ..KnowledgeBaseConfig::default()
            },
        ),
    ];

    let retrievers = build_knowledge_base_retrievers(&config, workspace_root.path())
        .await
        .expect("disabled and unwired sources should not fail bootstrap");

    assert!(retrievers.is_empty());
    assert!(
        !disabled_path.exists(),
        "disabled sqlite knowledge bases should not create database files"
    );
}

#[tokio::test]
async fn runtime_can_replace_knowledge_base_retrievers_after_config_refresh() {
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory store");
    let runtime = crate::LocalRuntime::new(store, agent_models::FakeModelClient::new(vec![]));
    assert!(runtime.knowledge_base_retrievers_snapshot().is_empty());

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    let retriever = agent_memory::SqliteFtsKnowledgeBase::new(
        "company-docs",
        pool,
        agent_memory::SqliteFtsKnowledgeBaseConfig::default(),
    )
    .await
    .unwrap();
    let mut retrievers: std::collections::HashMap<
        String,
        std::sync::Arc<dyn agent_memory::WorkspaceRetriever>,
    > = std::collections::HashMap::new();
    retrievers.insert("company-docs".into(), std::sync::Arc::new(retriever));

    runtime.replace_knowledge_base_retrievers(retrievers);

    assert!(runtime
        .knowledge_base_retrievers_snapshot()
        .contains_key("company-docs"));
}
