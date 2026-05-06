use agent_mcp::catalog::{
    CatalogQuery, EnvVarSpec, InstallSpec, RuntimeKind, RuntimeRequirement, ServerEntry, TrustLevel,
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
