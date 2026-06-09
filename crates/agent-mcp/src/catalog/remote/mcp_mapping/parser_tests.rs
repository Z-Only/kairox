use super::*;
use crate::catalog::{InstallSpec, RuntimeKind, TrustLevel};

fn minimal_server(name: &str) -> McpServerWrapper {
    McpServerWrapper {
        server: McpServer {
            name: name.to_string(),
            title: None,
            description: None,
            version: None,
            website_url: None,
            remotes: Vec::new(),
            packages: Vec::new(),
            repository: None,
        },
        meta: None,
    }
}

fn npm_package(identifier: &str) -> McpPackage {
    McpPackage {
        registry_type: "npm".to_string(),
        identifier: identifier.to_string(),
        _version: None,
        transport: None,
        environment_variables: Vec::new(),
    }
}

fn pypi_package(identifier: &str) -> McpPackage {
    McpPackage {
        registry_type: "pypi".to_string(),
        identifier: identifier.to_string(),
        _version: None,
        transport: None,
        environment_variables: Vec::new(),
    }
}

// ── first_sentence ──────────────────────────────────────────────────

#[test]
fn first_sentence_stops_at_period() {
    assert_eq!(
        first_sentence("Hello world. More text.", 200),
        "Hello world"
    );
}

#[test]
fn first_sentence_stops_at_newline() {
    assert_eq!(first_sentence("Line one\nLine two", 200), "Line one");
}

#[test]
fn first_sentence_truncates_long_text() {
    let long = "A".repeat(300);
    let result = first_sentence(&long, 10);
    assert!(result.chars().count() <= 11); // 10 + ellipsis
    assert!(result.ends_with('…'));
}

#[test]
fn first_sentence_returns_full_short_text() {
    assert_eq!(first_sentence("Short", 200), "Short");
}

#[test]
fn first_sentence_trims_whitespace() {
    assert_eq!(first_sentence("  Hello  ", 200), "Hello");
}

// ── is_latest ───────────────────────────────────────────────────────

#[test]
fn is_latest_true_when_flag_set() {
    let meta = Some(serde_json::json!({
        "io.modelcontextprotocol.registry/official": {"isLatest": true}
    }));
    assert!(is_latest(&meta));
}

#[test]
fn is_latest_false_when_flag_unset() {
    let meta = Some(serde_json::json!({
        "io.modelcontextprotocol.registry/official": {"isLatest": false}
    }));
    assert!(!is_latest(&meta));
}

#[test]
fn is_latest_defaults_true_when_no_meta() {
    assert!(is_latest(&None));
}

#[test]
fn is_latest_defaults_true_when_missing_flag() {
    let meta = Some(serde_json::json!({"other": "data"}));
    assert!(is_latest(&meta));
}

// ── infer_runtime_from_package ──────────────────────────────────────

#[test]
fn infer_runtime_npm_is_node() {
    let pkg = npm_package("@example/server");
    let req = infer_runtime_from_package(&pkg).unwrap();
    assert_eq!(req.kind, RuntimeKind::Node);
}

#[test]
fn infer_runtime_pypi_is_python() {
    let pkg = pypi_package("my-server");
    let req = infer_runtime_from_package(&pkg).unwrap();
    assert_eq!(req.kind, RuntimeKind::Python);
}

#[test]
fn infer_runtime_unknown_returns_none() {
    let pkg = McpPackage {
        registry_type: "cargo".to_string(),
        identifier: "my-crate".to_string(),
        _version: None,
        transport: None,
        environment_variables: Vec::new(),
    };
    assert!(infer_runtime_from_package(&pkg).is_none());
}

// ── build_install_from_package ──────────────────────────────────────

#[test]
fn build_install_npm_uses_npx() {
    let pkg = npm_package("@example/server");
    let install = build_install_from_package(&pkg);
    match install {
        InstallSpec::Stdio { command, args, .. } => {
            assert_eq!(command, "npx");
            assert_eq!(args, vec!["-y", "@example/server"]);
        }
        _ => panic!("expected Stdio install"),
    }
}

#[test]
fn build_install_pypi_uses_uvx() {
    let pkg = pypi_package("my-server");
    let install = build_install_from_package(&pkg);
    match install {
        InstallSpec::Stdio { command, args, .. } => {
            assert_eq!(command, "uvx");
            assert_eq!(args, vec!["my-server"]);
        }
        _ => panic!("expected Stdio install"),
    }
}

#[test]
fn build_install_unknown_registry_uses_identifier() {
    let pkg = McpPackage {
        registry_type: "cargo".to_string(),
        identifier: "my-binary".to_string(),
        _version: None,
        transport: None,
        environment_variables: Vec::new(),
    };
    let install = build_install_from_package(&pkg);
    match install {
        InstallSpec::Stdio { command, args, .. } => {
            assert_eq!(command, "my-binary");
            assert!(args.is_empty());
        }
        _ => panic!("expected Stdio install"),
    }
}

// ── map_mcp_to_entry ────────────────────────────────────────────────

#[test]
fn map_minimal_server_produces_entry() {
    let wrapper = minimal_server("com.example/test");
    let entry = map_mcp_to_entry("test-source", &wrapper, TrustLevel::Community).unwrap();

    assert_eq!(entry.id, "com.example/test");
    assert_eq!(entry.source, "test-source");
    assert_eq!(entry.display_name, "test"); // rsplit '/' fallback
    assert_eq!(entry.trust, TrustLevel::Community);
}

#[test]
fn map_server_with_title_uses_title() {
    let mut wrapper = minimal_server("com.example/test");
    wrapper.server.title = Some("My Test Server".into());
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Community).unwrap();

    assert_eq!(entry.display_name, "My Test Server");
}

#[test]
fn map_server_with_description_builds_summary() {
    let mut wrapper = minimal_server("test");
    wrapper.server.description = Some("A great server. With many features.".into());
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Community).unwrap();

    assert_eq!(entry.summary, "A great server");
    assert_eq!(entry.description, "A great server. With many features.");
}

#[test]
fn map_server_with_remote_prefers_remote_install() {
    let mut wrapper = minimal_server("test");
    wrapper.server.remotes = vec![McpRemote {
        transport_type: "streamable-http".into(),
        url: "https://api.example.com/mcp".into(),
        headers: Vec::new(),
    }];
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Community).unwrap();

    match entry.install {
        InstallSpec::StreamableHttp { url, .. } => {
            assert_eq!(url, "https://api.example.com/mcp");
        }
        _ => panic!("expected StreamableHttp install"),
    }
}

#[test]
fn map_server_with_sse_remote() {
    let mut wrapper = minimal_server("test");
    wrapper.server.remotes = vec![McpRemote {
        transport_type: "sse".into(),
        url: "https://api.example.com/sse".into(),
        headers: Vec::new(),
    }];
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Community).unwrap();

    match entry.install {
        InstallSpec::Sse { url, .. } => {
            assert_eq!(url, "https://api.example.com/sse");
        }
        _ => panic!("expected Sse install"),
    }
}

#[test]
fn map_server_falls_back_to_package_install() {
    let mut wrapper = minimal_server("test");
    wrapper.server.packages = vec![npm_package("@example/server")];
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Community).unwrap();

    match entry.install {
        InstallSpec::Stdio { command, .. } => assert_eq!(command, "npx"),
        _ => panic!("expected Stdio install"),
    }
}

#[test]
fn map_server_collects_env_vars() {
    let mut wrapper = minimal_server("test");
    wrapper.server.packages = vec![McpPackage {
        registry_type: "npm".into(),
        identifier: "@example/server".into(),
        _version: None,
        transport: None,
        environment_variables: vec![
            McpEnvVar {
                name: Some("API_KEY".into()),
                description: Some("The key".into()),
                is_required: Some(true),
                is_secret: Some(true),
            },
            McpEnvVar {
                name: Some("REGION".into()),
                description: None,
                is_required: Some(false),
                is_secret: Some(false),
            },
        ],
    }];
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Community).unwrap();

    assert_eq!(entry.default_env.len(), 2);
    assert_eq!(entry.default_env[0].key, "API_KEY");
    assert!(entry.default_env[0].required);
    assert!(entry.default_env[0].secret);
    assert_eq!(entry.default_env[1].key, "REGION");
    assert!(!entry.default_env[1].required);
}

#[test]
fn map_server_deduplicates_env_vars() {
    let mut wrapper = minimal_server("test");
    wrapper.server.packages = vec![
        McpPackage {
            registry_type: "npm".into(),
            identifier: "a".into(),
            _version: None,
            transport: None,
            environment_variables: vec![McpEnvVar {
                name: Some("KEY".into()),
                description: None,
                is_required: None,
                is_secret: None,
            }],
        },
        McpPackage {
            registry_type: "npm".into(),
            identifier: "b".into(),
            _version: None,
            transport: None,
            environment_variables: vec![McpEnvVar {
                name: Some("KEY".into()),
                description: Some("duplicate".into()),
                is_required: None,
                is_secret: None,
            }],
        },
    ];
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(entry.default_env.len(), 1);
}

#[test]
fn map_server_trust_clamped_by_ceiling() {
    let wrapper = minimal_server("test");
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Unverified).unwrap();
    assert_eq!(entry.trust, TrustLevel::Unverified);
}

#[test]
fn map_server_infers_runtime_requirements() {
    let mut wrapper = minimal_server("test");
    wrapper.server.packages = vec![
        npm_package("@example/a"),
        npm_package("@example/b"),
        pypi_package("c"),
    ];
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Community).unwrap();

    assert_eq!(entry.requirements.len(), 2);
    assert!(entry
        .requirements
        .iter()
        .any(|r| r.kind == RuntimeKind::Node));
    assert!(entry
        .requirements
        .iter()
        .any(|r| r.kind == RuntimeKind::Python));
}

#[test]
fn map_server_uses_website_url_as_homepage() {
    let mut wrapper = minimal_server("test");
    wrapper.server.website_url = Some("https://example.com".into());
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(entry.homepage.as_deref(), Some("https://example.com"));
}

#[test]
fn map_server_falls_back_to_repo_url() {
    let mut wrapper = minimal_server("test");
    wrapper.server.repository = Some(McpRepository {
        url: Some("https://github.com/example/test".into()),
    });
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Community).unwrap();
    assert_eq!(
        entry.homepage.as_deref(),
        Some("https://github.com/example/test")
    );
}

#[test]
fn map_server_collects_remote_headers_as_env() {
    let mut wrapper = minimal_server("test");
    wrapper.server.remotes = vec![McpRemote {
        transport_type: "streamable-http".into(),
        url: "https://api.example.com".into(),
        headers: vec![McpRemoteHeader {
            name: Some("Authorization".into()),
            description: Some("Bearer token".into()),
            is_required: Some(true),
            is_secret: Some(true),
        }],
    }];
    let entry = map_mcp_to_entry("src", &wrapper, TrustLevel::Community).unwrap();

    assert_eq!(entry.default_env.len(), 1);
    assert_eq!(entry.default_env[0].key, "Authorization");
    assert!(entry.default_env[0].required);
    assert!(entry.default_env[0].secret);
}
