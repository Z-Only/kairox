use agent_mcp::catalog::builtin::BuiltinCatalogProvider;
use agent_mcp::catalog::{
    CatalogProvider, CatalogQuery, EnvVarSpec, InstallSpec, RuntimeKind, RuntimeRequirement,
    ServerEntry, TrustLevel,
};
use std::collections::BTreeMap;

#[test]
fn server_entry_round_trips_through_json() {
    let entry = ServerEntry {
        id: "filesystem".into(),
        source: "builtin".into(),
        display_name: "Filesystem".into(),
        summary: "summary".into(),
        description: "desc".into(),
        categories: vec!["filesystem".into()],
        tags: vec!["files".into()],
        author: Some("MCP".into()),
        homepage: None,
        version: Some("0.6.0".into()),
        install: InstallSpec::Stdio {
            command: "npx".into(),
            args: vec![
                "-y".into(),
                "@modelcontextprotocol/server-filesystem".into(),
            ],
            env: BTreeMap::new(),
            cwd: None,
        },
        requirements: vec![RuntimeRequirement {
            kind: RuntimeKind::Node,
            min_version: Some(">=18.0.0".into()),
            install_hint: Some("https://nodejs.org".into()),
        }],
        trust: TrustLevel::Verified,
        default_env: vec![EnvVarSpec {
            key: "WORKSPACE_PATH".into(),
            label: "Workspace path".into(),
            description: "directory the server can read".into(),
            required: true,
            secret: false,
            default: Some("~".into()),
        }],
        icon: Some("📁".into()),
    };

    let json = serde_json::to_string(&entry).expect("serialize");
    let back: ServerEntry = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(entry, back);
}

#[test]
fn catalog_query_default_is_open() {
    let q = CatalogQuery::default();
    assert!(q.keyword.is_none());
    assert!(q.category.is_none());
    assert!(q.trust_min.is_none());
    assert!(q.source.is_none());
    assert!(q.limit.is_none());
}

#[test]
fn builtin_catalog_json_parses() {
    const JSON: &str = include_str!("../src/catalog/data/builtin-catalog.json");

    #[derive(serde::Deserialize)]
    struct Doc {
        schema_version: String,
        entries: Vec<agent_mcp::catalog::ServerEntry>,
    }
    let doc: Doc = serde_json::from_str(JSON).expect("builtin catalog must be valid JSON");
    assert_eq!(doc.schema_version, "1");
    assert_eq!(doc.entries.len(), 24, "expected 24 curated entries");

    let mut seen = std::collections::HashSet::new();
    for entry in &doc.entries {
        assert!(seen.insert(entry.id.clone()), "duplicate id: {}", entry.id);
        assert_eq!(entry.source, "builtin");
        assert!(!entry.display_name.is_empty());
        assert!(!entry.summary.is_empty());
        assert!(
            !entry.description.is_empty(),
            "entry {} has empty description",
            entry.id
        );
        assert!(
            entry.summary.chars().count() <= 200,
            "summary too long for {}",
            entry.id
        );
    }
}

#[tokio::test]
async fn builtin_provider_lists_all_when_query_empty() {
    let p = BuiltinCatalogProvider::new().expect("builtin loads");
    let items = p.list(&CatalogQuery::default()).await.unwrap();
    assert_eq!(items.len(), 24);
    assert_eq!(p.source_id(), "builtin");
}

#[tokio::test]
async fn builtin_provider_filters_by_keyword_and_trust() {
    let p = BuiltinCatalogProvider::new().unwrap();
    let only_verified = p
        .list(&CatalogQuery {
            trust_min: Some(TrustLevel::Verified),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(!only_verified.is_empty());
    assert!(only_verified
        .iter()
        .all(|e| e.trust == TrustLevel::Verified));

    let by_kw = p
        .list(&CatalogQuery {
            keyword: Some("file".into()),
            ..Default::default()
        })
        .await
        .unwrap();
    assert!(by_kw.iter().any(|e| e.id == "filesystem"));
}

#[tokio::test]
async fn builtin_provider_get_returns_none_for_unknown() {
    let p = BuiltinCatalogProvider::new().unwrap();
    assert!(p.get("does-not-exist").await.unwrap().is_none());
    assert!(p.get("filesystem").await.unwrap().is_some());
}
