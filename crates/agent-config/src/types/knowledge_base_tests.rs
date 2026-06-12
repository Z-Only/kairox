use super::*;

#[test]
fn knowledge_base_kind_accepts_legacy_aliases() {
    let sqlite: KnowledgeBaseKind = serde_json::from_str(r#""sqlite""#).unwrap();
    let bedrock: KnowledgeBaseKind = serde_json::from_str(r#""bedrock_knowledge_base""#).unwrap();

    assert_eq!(sqlite, KnowledgeBaseKind::SqliteFts);
    assert_eq!(bedrock, KnowledgeBaseKind::BedrockKnowledgeBase);
}

#[test]
fn knowledge_base_kind_serializes_canonical_names() {
    assert_eq!(
        serde_json::to_string(&KnowledgeBaseKind::SqliteFts).unwrap(),
        r#""sqlite_fts""#
    );
    assert_eq!(
        serde_json::to_string(&KnowledgeBaseKind::BedrockKnowledgeBase).unwrap(),
        r#""bedrock""#
    );
    assert_eq!(
        serde_json::to_string(&KnowledgeBaseKind::Pinecone).unwrap(),
        r#""pinecone""#
    );
}

#[test]
fn knowledge_base_config_default_is_enabled_sqlite_fts() {
    let cfg = KnowledgeBaseConfig::default();

    assert_eq!(cfg.kind, KnowledgeBaseKind::SqliteFts);
    assert!(cfg.enabled);
    assert!(cfg.profile_aliases.is_empty());
    assert!(cfg.path.is_none());
    assert!(cfg.endpoint.is_none());
    assert!(cfg.api_key_env.is_none());
    assert!(cfg.region.is_none());
    assert!(cfg.knowledge_base_id.is_none());
    assert!(cfg.index_name.is_none());
    assert!(cfg.namespace.is_none());
    assert!(cfg.collection.is_none());
    assert!(cfg.table.is_none());
    assert!(cfg.id_column.is_none());
    assert!(cfg.title_column.is_none());
    assert!(cfg.content_column.is_none());
    assert!(cfg.workspace_id_column.is_none());
    assert!(cfg.max_results.is_none());
    assert!(cfg.min_score.is_none());
}

#[test]
fn knowledge_base_config_deserializes_backend_fields() {
    let cfg: KnowledgeBaseConfig = toml::from_str(
        r#"
kind = "bedrock"
enabled = false
profile_aliases = ["fast", "wide"]
region = "us-east-1"
knowledge_base_id = "kb-123"
max_results = 8
min_score = 0.42
"#,
    )
    .unwrap();

    assert_eq!(cfg.kind, KnowledgeBaseKind::BedrockKnowledgeBase);
    assert!(!cfg.enabled);
    assert_eq!(cfg.profile_aliases, vec!["fast", "wide"]);
    assert_eq!(cfg.region.as_deref(), Some("us-east-1"));
    assert_eq!(cfg.knowledge_base_id.as_deref(), Some("kb-123"));
    assert_eq!(cfg.max_results, Some(8));
    assert_eq!(cfg.min_score, Some(0.42));
}

#[test]
fn knowledge_base_config_deserializes_sql_columns() {
    let cfg: KnowledgeBaseConfig = toml::from_str(
        r#"
kind = "sqlite"
path = ".kairox/kb.sqlite"
table = "documents"
id_column = "id"
title_column = "title"
content_column = "body"
workspace_id_column = "workspace"
"#,
    )
    .unwrap();

    assert_eq!(cfg.kind, KnowledgeBaseKind::SqliteFts);
    assert_eq!(cfg.path.as_deref(), Some(".kairox/kb.sqlite"));
    assert_eq!(cfg.table.as_deref(), Some("documents"));
    assert_eq!(cfg.id_column.as_deref(), Some("id"));
    assert_eq!(cfg.title_column.as_deref(), Some("title"));
    assert_eq!(cfg.content_column.as_deref(), Some("body"));
    assert_eq!(cfg.workspace_id_column.as_deref(), Some("workspace"));
    assert!(cfg.enabled);
}
